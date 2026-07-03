use crate::{vga_buffer, keyboard, cpuinfo, meminfo, cmos, reboot, acpi, allocator};
use alloc::vec::Vec;
use spin::Lazy;
use spin::Mutex;

const BANNER: &str = include_str!("banner.txt");
const PROMPT: &str = "Hammerhead>_ ";

static KERNEL_LOGS: Lazy<Mutex<Vec<&'static str>>> = Lazy::new(|| {
    let mut logs = Vec::new();
    logs.push("[INFO] Core exception tables GDT and IDT successfully loaded.");
    logs.push("[INFO] Chained 8259 programmable interrupt controllers initialized.");
    logs.push("[INFO] Global heap allocation layer verified across virtual pages.");
    logs.push("[INFO] External hardware interrupt lines enabled at CPU level.");
    Mutex::new(logs)
});

pub fn run() -> ! {
    x86_64::instructions::interrupts::enable();
    vga_buffer::print_something(BANNER);
    vga_buffer::print_something("\n");

    loop {
        vga_buffer::print_something(PROMPT);
        let mut line = [0u8; 256];
        let mut pos = 0;

        loop {
            if let Some(c) = keyboard::read_key() {
                if c == '\n' {
                    vga_buffer::print_something("\n");
                    break;
                } else if c == '\x08' {
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
                x86_64::instructions::hlt();
            }
        }

        let input_str = core::str::from_utf8(&line[..pos]).unwrap_or("");
        let mut parts = input_str.trim().split_whitespace();
        let command = parts.next().unwrap_or("");

        match command {
            "" => {}
            "help" => vga_buffer::print_something(
                "Commands: help, clear, echo [text], shutdown, reboot, halt, cpuinfo, meminfo, date, tsc, ps, sysload, dmesg\n"
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
            "tsc" => {
                let tsc = unsafe { core::arch::x86_64::_rdtsc() };
                vga_buffer::print_fmt(format_args!("TSC Ticks: {}\n", tsc));
            }
            "ps" => {
                vga_buffer::print_something("PID   TASK NAME      STATUS     STACK CURRENT POINTER\n");
                let current_rsp: u64;
                unsafe {
                    core::arch::asm!("mov {}, rsp", out(reg) current_rsp);
                }
                vga_buffer::print_fmt(format_args!("0     kernel_shell   RUNNING    0x{:016X}\n", current_rsp));
                vga_buffer::print_something("1     task_alpha     READY      SYSTEM_MANAGED\n");
                vga_buffer::print_something("2     task_beta      READY      SYSTEM_MANAGED\n");
            }
            "sysload" => {
                let tsc = unsafe { core::arch::x86_64::_rdtsc() };
                vga_buffer::print_fmt(format_args!("CPU Performance Ticks (TSC): {}\n", tsc));
                vga_buffer::print_fmt(format_args!(
                    "Heap Boundaries Mapped: 0x{:X} - 0x{:X} ({} Bytes Allocated)\n",
                    allocator::HEAP_START,
                    allocator::HEAP_START + allocator::HEAP_SIZE,
                    allocator::HEAP_SIZE
                ));
            }
            "dmesg" => {
                let logs = KERNEL_LOGS.lock();
                for entry in logs.iter() {
                    vga_buffer::print_something(entry);
                    vga_buffer::print_something("\n");
                }
            }
            other => {
                vga_buffer::print_something(other);
                vga_buffer::print_something(": command not found\n");
                KERNEL_LOGS.lock().push("[ERROR] Executive interface dropped unrecognized command input.");
            }
        }
    }
}