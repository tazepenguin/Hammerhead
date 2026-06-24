use pc_keyboard::{DecodedKey, HandleControl, Keyboard, ScancodeSet1, layouts};
use spin::{Lazy, Mutex};
use x86_64::instructions::port::Port;

static KEYBOARD: Lazy<Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>>> = Lazy::new(|| {
    Mutex::new(Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore))
});

pub fn read_key() -> Option<char> {
    let mut status_port = Port::new(0x64);
    let status: u8 = unsafe { status_port.read() };
    
    // Bit 0 must be 1 if there is unread data waiting in the buffer
    if status & 0x01 == 0 {
        return None;
    }

    let mut data_port = Port::new(0x60);
    let scancode: u8 = unsafe { data_port.read() };
    
    let mut kb = KEYBOARD.lock();
    if let Ok(Some(event)) = kb.add_byte(scancode) {
        if let Some(DecodedKey::Unicode(c)) = kb.process_keyevent(event) {
            return Some(c);
        }
    }
    None
}