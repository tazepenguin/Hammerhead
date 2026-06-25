#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

use core::panic::PanicInfo;
use bootloader::{entry_point, BootInfo};
use spin::Once;

mod vga_buffer;
mod keyboard;
mod shell;
mod cpuinfo;
mod meminfo;
mod cmos;
mod reboot;
mod memory;    
mod allocator; 
mod gdt;        
mod interrupts; 

static BOOT_INFO: Once<&'static BootInfo> = Once::new();

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    BOOT_INFO.call_once(|| boot_info);

    vga_buffer::init();
    vga_buffer::print_banner();
    vga_buffer::print_something("\n\n");

    // Initialize core hardware exception stacks and mappings
    gdt::init();
    interrupts::init_idt();
    vga_buffer::print_something("GDT & IDT Exception Structures: OK\n");

    // Initialize the PIC hardware lines
    unsafe { interrupts::PICS.lock().initialize(); }
    vga_buffer::print_something("Programmable Interrupt Controller: OK\n");

    let phys_mem_offset = x86_64::VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        memory::BootInfoFrameAllocator::new(&boot_info.memory_map)
    };

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("Failed to initialize Hammerhead Heap Allocator Space");
    vga_buffer::print_something("Heap Allocator: OK [Initialized 128 KiB Space]\n");

    vga_buffer::print_something("Enabling CPU hardware interrupt routing...\n\n");
    
    // Unmask hardware interrupts at the CPU level
    x86_64::instructions::interrupts::enable(); 

    shell::run();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga_buffer::print_something("\nKERNEL PANIC: ");
    vga_buffer::print_something(info.message().as_str().unwrap_or("???"));
    loop {
        x86_64::instructions::hlt();
    }
}