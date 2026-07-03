use crate::{vga_buffer, keyboard, cpuinfo, meminfo, cmos, reboot, acpi};

// Embed the banner at compile time.
// Create a file named `banner.txt` in your `src/` folder with any ASCII art.
const BANNER: &str = include_str!("banner.txt");

const PROMPT: &str = "Hammerhead>_ ";

pub fn run() -> ! {
    // We are entered via switch_context(), which was called from inside a
    // without_interrupts() closure in run_yield(). That closure never gets
    // to restore IF, so we arrive here with interrupts permanently disabled.
    // Re-enable them explicitly before doing anything else.
    x86_64::instructions::interrupts::enable();
    // 1. Show the banner when the shell starts
    vga_buffer::print_something(BANNER);
    vga_buffer::print_something("\n");

    loop {
        vga_buffer::print_something(PROMPT);
        let mut line = [0u8; 256];
        let mut pos = 0;

        // Inner input loop – handles backspace and Enter
        loop {
            if let Some(c) = keyboard::read_key() {
                if c == '\n' {
                    vga_buffer::print_something("\n");
                    break;
                } else if c == '\x08' {
                    if pos > 0 {
                        pos -= 1;
                        // Move left, erase, move left again
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
                // No character available – wait for the next interrupt
                x86_64::instructions::hlt();
            }
        }

        // Parse and run the command
        let input_str = core::str::from_utf8(&line[..pos]).unwrap_or("");
        let mut parts = input_str.trim().split_whitespace();
        let command = parts.next().unwrap_or("");

        match command {
            "" => {}
            "help" => vga_buffer::print_something(
                "Commands: help, clear, echo [text], shutdown, reboot, halt, cpuinfo, meminfo, date\n"
            ),
            "clear" => vga_buffer::clear_screen(),
            "echo" => {
                for arg in parts {
                    vga_buffer::print_something(arg);
                    vga_buffer::print_something(" ");
                }
                vga_buffer::print_something("\n");
            }
            "shutdown" => acpi::shutdown(),
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