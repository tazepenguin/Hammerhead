use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use spin::Lazy;
use pic8259::ChainedPics;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> = unsafe {
    spin::Mutex::new(ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET))
};

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

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(_stack_frame: InterruptStackFrame) {
    crate::vga_buffer::print_something("\n[INTERRUPT: BREAKPOINT TRAP]\n");
}

extern "x86-interrupt" fn double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("CRITICAL KERNEL DOUBLE FAULT!");
}

extern "x86-interrupt" fn timer_handler(_stack_frame: InterruptStackFrame) {
    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer.as_u8()); }
}

extern "x86-interrupt" fn keyboard_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

    static KEYBOARD: Lazy<spin::Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>>> =
        Lazy::new(|| {
            spin::Mutex::new(
                Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore)
            )
        });

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60u16);
    let scancode: u8 = unsafe { port.read() };

    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => {
                    crate::keyboard::add_char(character);
                }
                DecodedKey::RawKey(_key) => {
                    // Non-unicode keys (arrows, function keys, etc.) are
                    // silently dropped for now.  Extending this to handle
                    // cursor movement or command history is straightforward:
                    // add a parallel RawKey queue in keyboard.rs and drain
                    // it in shell.rs alongside read_key().
                    //
                    // We deliberately do NOT call print_fmt here: doing so
                    // while the shell might hold the WRITER spin-lock (even
                    // with without_interrupts protection) is unnecessary
                    // and was a latent deadlock risk.
                }
            }
        }
    }

    unsafe { PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8()); }
}