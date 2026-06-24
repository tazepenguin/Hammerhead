#![no_std]
#![no_main]

use core::panic::PanicInfo;

mod vga_buffer;
mod keyboard;
mod shell;
mod cpuinfo;
mod meminfo;
mod cmos;
mod reboot;

use bootloader::{entry_point, BootInfo};
use spin::Once;

static BOOT_INFO: Once<&'static BootInfo> = Once::new();

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    BOOT_INFO.call_once(|| boot_info);

    vga_buffer::init();
    vga_buffer::print_banner();
    
    // Creates the two-line space between the system info and the shell prompt
    vga_buffer::print_something("\n\n");

    shell::run();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga_buffer::print_something("KERNEL PANIC: ");
    vga_buffer::print_something(info.message().as_str().unwrap_or("???"));
    loop {
        x86_64::instructions::hlt();
    }
}