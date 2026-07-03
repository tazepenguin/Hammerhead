use spin::Mutex;
use spin::Lazy;
use core::ptr::{read_volatile, write_volatile};
use core::fmt;
use x86_64::instructions::interrupts;

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: u8,
}

struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    row_position: usize,
    column_position: usize,
    color_code: u8,
    buffer: *mut Buffer,
}

unsafe impl Send for Writer {}

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            0x08 => {
                if self.column_position > 0 {
                    self.column_position -= 1;
                } else if self.row_position > 0 {
                    self.row_position -= 1;
                    self.column_position = BUFFER_WIDTH - 1;
                }
                let blank = ScreenChar { ascii_character: b' ', color_code: self.color_code };
                unsafe {
                    let ptr = &mut (*self.buffer).chars[self.row_position][self.column_position]
                        as *mut ScreenChar;
                    write_volatile(ptr, blank);
                }
            }
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }
                let row = self.row_position;
                let col = self.column_position;
                unsafe {
                    let ptr = &mut (*self.buffer).chars[row][col] as *mut ScreenChar;
                    write_volatile(ptr, ScreenChar { ascii_character: byte, color_code: self.color_code });
                }
                self.column_position += 1;
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }
            }
        }
    }

    fn new_line(&mut self) {
        self.column_position = 0;
        if self.row_position < BUFFER_HEIGHT - 1 {
            self.row_position += 1;
        } else {
            self.scroll();
        }
    }

    fn scroll(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                unsafe {
                    // Both sides of the copy must be volatile: 0xb8000 is
                    // memory-mapped hardware, not ordinary RAM.
                    let src = &(*self.buffer).chars[row][col] as *const ScreenChar;
                    let dst = &mut (*self.buffer).chars[row - 1][col] as *mut ScreenChar;
                    write_volatile(dst, read_volatile(src));
                }
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.row_position = BUFFER_HEIGHT - 1;
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' | 0x08 => self.write_byte(byte),
                _ => self.write_byte(0xfe),
            }
        }
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar { ascii_character: b' ', color_code: self.color_code };
        for col in 0..BUFFER_WIDTH {
            unsafe {
                let ptr = &mut (*self.buffer).chars[row][col] as *mut ScreenChar;
                write_volatile(ptr, blank);
            }
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

pub static WRITER: Lazy<Mutex<Writer>> = Lazy::new(|| Mutex::new(Writer {
    row_position: 0,
    column_position: 0,
    color_code: 0x07,
    buffer: 0xb8000 as *mut Buffer,
}));

pub fn init() {
    clear_screen();
}

// All public print functions disable interrupts before acquiring WRITER.
//
// Why: the keyboard interrupt handler can call print_fmt() for RawKey events.
// If the shell is mid-print and holds the WRITER spin-lock when the interrupt
// fires, the handler would spin on WRITER with IF=0 — a classic deadlock.
// Disabling interrupts while holding WRITER guarantees the handler can never
// observe the lock as taken.

pub fn print_something(s: &str) {
    interrupts::without_interrupts(|| {
        WRITER.lock().write_string(s);
    });
}

pub fn print_char(c: char) {
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        if c.is_ascii() {
            writer.write_byte(c as u8);
        } else {
            writer.write_byte(0xfe);
        }
    });
}

pub fn print_fmt(args: fmt::Arguments) {
    use core::fmt::Write;
    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}

pub fn clear_screen() {
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        for row in 0..BUFFER_HEIGHT {
            writer.clear_row(row);
        }
        writer.row_position = 0;
        writer.column_position = 0;
    });
}