use core::fmt;
use spin::Mutex;
use x86_64::instructions::interrupts;

const BUFFER_ADDR: usize = 0xb8000;
const WIDTH: usize = 80;
const HEIGHT: usize = 25;

pub static WRITER: Mutex<Writer> = Mutex::new(Writer {
    row: 0,
    col: 0,
    fg: Color::Green,
    bg: Color::Black,
});

#[allow(dead_code)]
#[derive(Copy, Clone)]
#[repr(u8)]
enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

pub(crate) struct Writer {
    row: usize,
    col: usize,
    fg: Color,
    bg: Color,
}

impl Writer {
    fn buffer(&self) -> &'static mut [[u16; WIDTH]; HEIGHT] {
        unsafe { &mut *(BUFFER_ADDR as *mut _) }
    }

    fn new_line(&mut self) {
        self.col = 0;
        self.row += 1;
        if self.row >= HEIGHT {
            // Scroll up one line
            for r in 1..HEIGHT {
                for c in 0..WIDTH {
                    let val = self.buffer()[r][c];
                    self.buffer()[r - 1][c] = val;
                }
            }
            // Clear the last line
            for c in 0..WIDTH {
                self.buffer()[HEIGHT - 1][c] = 0x0f00 | b' ' as u16;
            }
            self.row = HEIGHT - 1;
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                b'\n' => self.new_line(),
                // INTERCEPT BACKSPACE: Move the cursor column back by 1 if possible
                b'\x08' => {
                    if self.col > 0 {
                        self.col -= 1;
                    }
                }
                byte => {
                    if self.col >= WIDTH {
                        self.new_line();
                    }
                    let color = ((self.bg as u8) << 4) | (self.fg as u8);
                    self.buffer()[self.row][self.col] = (color as u16) << 8 | byte as u16;
                    self.col += 1;
                }
            }
        }
    }

    pub fn write_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        self.write_string(c.encode_utf8(&mut buf));
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

pub fn init() {
    let mut writer = WRITER.lock();
    for row in 0..HEIGHT {
        for col in 0..WIDTH {
            writer.buffer()[row][col] = 0x0f00 | b' ' as u16;
        }
    }
    writer.row = 0;
    writer.col = 0;
}

pub fn print_banner() {
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writer.fg = Color::Green;
        writer.write_string(include_str!("banner.txt"));
    });
}

pub fn print_something(s: &str) {
    interrupts::without_interrupts(|| {
        WRITER.lock().write_string(s);
    });
}

pub fn print_char(c: char) {
    interrupts::without_interrupts(|| {
        WRITER.lock().write_char(c);
    });
}

pub fn clear_screen() {
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        for row in 0..HEIGHT {
            for col in 0..WIDTH {
                writer.buffer()[row][col] = 0x0f00 | b' ' as u16;
            }
        }
        writer.row = 0;
        writer.col = 0;
    });
}

pub fn print_fmt(args: fmt::Arguments) {
    interrupts::without_interrupts(|| {
        use core::fmt::Write;
        WRITER.lock().write_fmt(args).unwrap();
    });
}