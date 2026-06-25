#![no_std]
#![no_main]

extern crate alloc; // High-level linking wrapper for dynamic data types

use core::panic::PanicInfo;
use bootloader::{entry_point, BootInfo};
use spin::Once;
use alloc::vec::Vec;

mod vga_buffer;
mod keyboard;
mod shell;
mod cpuinfo;
mod meminfo;
mod cmos;
mod reboot;
mod memory;    // Added Memory module
mod allocator; // Added Allocator module

static BOOT_INFO: Once<&'static BootInfo> = Once::new();

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    BOOT_INFO.call_once(|| boot_info);

    vga_buffer::init();
    vga_buffer::print_banner();
    vga_buffer::print_something("\n\n");

    // 1. Extract physical offsets from the environment structures
    let phys_mem_offset = x86_64::VirtAddr::new(boot_info.physical_memory_offset);
    
    // 2. Map structural translation frames
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        memory::BootInfoFrameAllocator::new(&boot_info.memory_map)
    };

    // 3. Mount the dynamic heap structures
    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("Failed to initialize Hammerhead Heap Allocator Space");

    vga_buffer::print_something("Heap Allocator: OK [Initialized 128 KiB Space]\n");

    // 4. Test dynamic runtime vectors!
    let mut test_vector = Vec::new();
    for i in 0..5 {
        test_vector.push(i);
    }
    vga_buffer::print_something("Dynamic Vectors: OK [Successfully allocated on Heap]\n\n");

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