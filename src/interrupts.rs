use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use x86_64::instructions::port::Port;
use core::sync::atomic::{AtomicU64, Ordering};
use spin::Lazy;
use pic8259::ChainedPics;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> = unsafe {
    spin::Mutex::new(ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET))
};

// ── Tick counter ─────────────────────────────────────────────────────────────

/// PIT frequency we program in `init_pit`.  All uptime calculations use this.
pub const PIT_HZ: u64 = 100;

static TICK_COUNT: AtomicU64 = AtomicU64::new(0);

/// Monotonic tick counter incremented by the timer interrupt at `PIT_HZ`.
pub fn ticks() -> u64 {
    TICK_COUNT.load(Ordering::Relaxed)
}

// ── PIT setup ────────────────────────────────────────────────────────────────

/// Program Intel 8253/8254 PIT channel 0 to fire IRQ0 at `hz` Hz.
///
/// The PIT input clock is 1 193 182 Hz.  We use mode 3 (square-wave
/// generator) so the divisor controls the output frequency.
///
/// Call this *after* `PICS.initialize()` and *before* `interrupts::enable()`.
pub fn init_pit(hz: u32) {
    let divisor = (1_193_182u32 / hz).min(0xFFFF) as u16;
    unsafe {
        // Command byte: channel 0 | lo/hi byte access | mode 3 | binary
        Port::<u8>::new(0x43).write(0x36);
        Port::<u8>::new(0x40).write((divisor & 0xFF) as u8);       // low byte
        Port::<u8>::new(0x40).write(((divisor >> 8) & 0xFF) as u8); // high byte
    }
}

// ── IDT ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer    = PIC_1_OFFSET,
    Keyboard = PIC_1_OFFSET + 1,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 { self as u8 }
    fn as_usize(self) -> usize { self as usize }
}

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    unsafe {
        idt.double_fault
            .set_handler_fn(double_fault_handler)
            .set_stack_index(crate::gdt::DOUBLE_FAULT_IST_INDEX);
    }
    idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_handler);
    idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_handler);
    idt
});

pub fn init_idt() { IDT.load(); }

// ── Handlers ─────────────────────────────────────────────────────────────────

extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
    crate::vga_buffer::print_something("\n[BREAKPOINT TRAP — returned]\n");
}

extern "x86-interrupt" fn double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("CRITICAL KERNEL DOUBLE FAULT!");
}

extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    TICK_COUNT.fetch_add(1, Ordering::Relaxed);
    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8()); }
}

extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

    static KEYBOARD: Lazy<spin::Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>>> =
        Lazy::new(|| spin::Mutex::new(
            Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore)
        ));

    let mut keyboard = KEYBOARD.lock();
    let scancode: u8 = unsafe { Port::new(0x60u16).read() };

    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(c) => crate::keyboard::add_char(c),
                DecodedKey::RawKey(_)  => { /* silently drop — see interrupts.rs comment */ }
            }
        }
    }
    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8()); }
}