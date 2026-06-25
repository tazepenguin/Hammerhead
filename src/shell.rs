use crate::{vga_buffer, keyboard, cpuinfo, meminfo, cmos, reboot};

const PROMPT: &str = "Hammerhead> ";

pub fn run() -> ! {
    loop {
        vga_buffer::print_something(PROMPT);
        let mut line = [0u8; 256];
        let mut pos = 0;

        loop {
            if let Some(c) = keyboard::read_key() {
                if c == '\n' {
                    vga_buffer::print_something("\n");
                    break;
                } else if c == '\x08' { // Backspace
                    if pos > 0 {
                        pos -= 1;
                        vga_buffer::print_something("\x08 \x08");
                    }
                } else {
                    if pos < 255 {
                        line[pos] = c as u8;
                        pos += 1;
                        vga_buffer::print_char(c);
                    }
                }
            } else {
                // Safe hardware sleep while waiting for keypresses
                x86_64::instructions::hlt();
            }
        }

        let input_str = core::str::from_utf8(&line[..pos]).unwrap_or("");
        
        // Split the input into an iterator of words
        let mut parts = input_str.trim().split_whitespace();
        
        // The first word is the command. If empty, skip processing.
        let command = parts.next().unwrap_or("");

        match command {
            "" => {} // User just hit enter
            "help" => vga_buffer::print_something(
                "Commands: help, clear, echo [text], shutdown, reboot, halt, cpuinfo, meminfo, date\n"
            ),
            "clear" => vga_buffer::clear_screen(),
            "echo" => {
                // Loop through all remaining words passed as arguments
                for arg in parts {
                    vga_buffer::print_something(arg);
                    vga_buffer::print_something(" ");
                }
                vga_buffer::print_something("\n");
            }
            "shutdown" => shut_down(),
            "reboot" => reboot::reboot(),
            "halt" => reboot::halt(),
            "cpuinfo" => cpuinfo::run(),
            "meminfo" => meminfo::run(),
            "date" => cmos::run(),
            other => {
                vga_buffer::print_something(other);
                vga_buffer::print_something(": command not found\n");
            }
        }
    }
}

fn shut_down() -> ! {
    use x86_64::instructions::port::Port;
    unsafe {
        Port::new(0x604).write(0x2000u16);
    }
    loop {
        x86_64::instructions::hlt();
    }
}