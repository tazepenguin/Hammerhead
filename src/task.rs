use alloc::boxed::Box;
use alloc::collections::VecDeque;
use spin::Lazy;
use spin::Mutex;

/// The execution state of a thread (the preserved CPU registers)
#[derive(Default, Clone, Copy)]
#[repr(C)]
struct TaskContext {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbx: u64,
    rbp: u64,
    rip: u64, // Instruction pointer (where to resume execution)
}

pub struct Task {
    id: usize,
    stack: Box<[u8]>,
    rsp: u64, // Current stack pointer for this task
}

impl Task {
    pub fn new(id: usize, entry_point: fn() -> !) -> Self {
        const STACK_SIZE: usize = 4096 * 4; // 16 KiB Stack
        let mut stack = Box::new([0u8; STACK_SIZE]);

        // Calculate top of the stack (x86_64 stacks grow downward)
        let stack_top = &stack[STACK_SIZE - 1] as *const u8 as usize;
        
        // Align stack to 16 bytes for System V ABI compliance
        let aligned_top = (stack_top & !0xF) - 8;

        // Allocate space right on the stack for our initial context state
        let context_ptr = (aligned_top - core::mem::size_of::<TaskContext>()) as *mut TaskContext;

        unsafe {
            context_ptr.write(TaskContext {
                rip: entry_point as u64,
                ..Default::default() // Clear all other registers to 0
            });
        }

        Task {
            id,
            stack,
            rsp: context_ptr as u64,
        }
    }
}

pub struct Scheduler {
    ready_queue: VecDeque<Task>,
    current_task: Option<Task>,
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler {
            ready_queue: VecDeque::new(),
            current_task: None,
        }
    }

    pub fn add_task(&mut self, task: Task) {
        self.ready_queue.push_back(task);
    }

    pub fn switch(&mut self) {
        if self.ready_queue.is_empty() {
            return; // Nowhere else to go
        }

        // Pull the next task off the queue
        if let Some(next_task) = self.ready_queue.pop_front() {
            // Take the currently running task out
            let old_task = self.current_task.take();
            
            // Set the new running task
            self.current_task = Some(next_task);

            unsafe {
                let mut old_rsp: u64 = 0;
                let old_rsp_ptr: *mut u64 = &mut old_rsp;
                let new_rsp = self.current_task.as_ref().unwrap().rsp;

                // Execute raw hardware context switch
                switch_context(old_rsp_ptr, new_rsp);

                // When we eventually switch back here, save where our old stack ended up
                if let Some(mut completed_old) = old_task {
                    completed_old.rsp = old_rsp;
                    self.ready_queue.push_back(completed_old);
                }
            }
        }
    }
}

pub static SCHEDULER: Lazy<Mutex<Scheduler>> = Lazy::new(|| Mutex::new(Scheduler::new()));

/// Voluntarily yield the CPU to the next thread
pub fn run_yield() {
    x86_64::instructions::interrupts::without_interrupts(|| {
        SCHEDULER.lock().switch();
    });
}

/// The raw assembly assembly routine that shifts the CPU stack pointer
#[naked]
unsafe extern "C" fn switch_context(old_rsp: *mut u64, new_rsp: u64) {
    // Under System V AMD64 ABI calling conventions:
    // rdi = first argument (old_rsp pointer)
    // rsi = second argument (new_rsp value)
    core::arch::asm!(
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov [rdi], rsp", // Save current CPU stack pointer into old_rsp
        "mov rsp, rsi",   // LOAD the new task's stack pointer into CPU
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        "ret",            // Jumps to the instruction pointer saved on the new stack
        options(noreturn)
    );
}