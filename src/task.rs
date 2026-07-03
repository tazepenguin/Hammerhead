use alloc::boxed::Box;
use alloc::collections::VecDeque;
use core::sync::atomic::AtomicU64;
use spin::Lazy;
use spin::Mutex;

/// The callee-saved registers that switch_context preserves across a context switch.
#[derive(Default, Clone, Copy)]
#[repr(C)]
struct TaskContext {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
    rip: u64, // where to resume (written as the synthetic return address)
}

pub struct Task {
    _id: usize,
    _stack: Box<[u8]>,
    pub rsp: u64, // saved stack pointer, updated on every context switch out
}

impl Task {
    pub fn new(id: usize, entry_point: fn() -> !) -> Self {
        const STACK_SIZE: usize = 4096 * 4; // 16 KiB
        let stack = Box::new([0u8; STACK_SIZE]);

        // x86_64 stacks grow downward; top = last byte.
        let stack_top = &stack[STACK_SIZE - 1] as *const u8 as usize;

        // System V AMD64 ABI: rsp must be 16-byte aligned *before* a call
        // instruction pushes the return address, so we subtract 8 first.
        let aligned_top = (stack_top & !0xF) - 8;

        // Carve out space for the initial TaskContext on the stack.
        let context_ptr = (aligned_top - core::mem::size_of::<TaskContext>()) as *mut TaskContext;

        unsafe {
            context_ptr.write(TaskContext {
                rip: entry_point as u64,
                ..Default::default()
            });
        }

        Task { _id: id, _stack: stack, rsp: context_ptr as u64 }
    }
}

pub struct Scheduler {
    ready_queue: VecDeque<Box<Task>>,
    current_task: Option<Box<Task>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler { ready_queue: VecDeque::new(), current_task: None }
    }

    pub fn add_task(&mut self, task: Task) {
        self.ready_queue.push_back(Box::new(task));
    }
}

pub static SCHEDULER: Lazy<Mutex<Scheduler>> = Lazy::new(|| Mutex::new(Scheduler::new()));

// Storage for the boot context's RSP when it yields for the first time.
// AtomicU64::as_ptr() gives us the *mut u64 that switch_context writes through,
// without needing `static mut` (which triggers the mutable-static lint and
// requires an unsafe block at every access site in Rust ≥ 2024).
static BOOT_RSP: AtomicU64 = AtomicU64::new(0);

/// Cooperatively yield the CPU to the next scheduled task.
pub fn run_yield() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        let (old_rsp_ptr, new_rsp) = {
            let mut sched = SCHEDULER.lock();
            if sched.ready_queue.is_empty() {
                return;
            }

            let next_task = sched.ready_queue.pop_front().unwrap();
            let old_task  = sched.current_task.take();
            sched.current_task = Some(next_task);

            // If there's no current task (boot context), save into BOOT_RSP.
            // AtomicU64 is guaranteed to have the same in-memory layout as u64,
            // so the cast below is sound.
            let old_ptr: *mut u64 = match old_task.as_ref() {
                Some(task) => &task.rsp as *const u64 as *mut u64,
                None       => BOOT_RSP.as_ptr(),
            };

            let new_val = sched.current_task.as_ref().unwrap().rsp;

            if let Some(task) = old_task {
                sched.ready_queue.push_back(task);
            }

            (old_ptr, new_val)
        }; // MutexGuard released here — lock is open before the stack switch.

        unsafe { switch_context(old_rsp_ptr, new_rsp); }
    });
}

/// Low-level context switch: saves callee-saved registers onto the current
/// stack, writes the current rsp to `*old_rsp`, then loads `new_rsp` and
/// restores the next task's registers.  The final `ret` pops rip, which for a
/// freshly-created task lands at its `entry_point`.
#[unsafe(naked)]
unsafe extern "C" fn switch_context(old_rsp: *mut u64, new_rsp: u64) {
    core::arch::naked_asm!(
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov [rdi], rsp",   // save current stack pointer → *old_rsp
        "mov rsp, rsi",     // load next task's stack pointer
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "ret",              // for a new task: pops entry_point as return address
    );
}