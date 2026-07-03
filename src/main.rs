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
mod task;
mod acpi;

static BOOT_INFO: Once<&'static BootInfo> = Once::new();

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    BOOT_INFO.call_once(|| boot_info);

    // Clear the screen; the banner is printed by the shell task when it starts.
    vga_buffer::init();

    gdt::init();
    interrupts::init_idt();
    vga_buffer::print_something("GDT & IDT Exception Structures: OK\n");

    unsafe { interrupts::PICS.lock().initialize(); }
    vga_buffer::print_something("Programmable Interrupt Controller: OK\n");

    let phys_mem_offset = x86_64::VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        memory::BootInfoFrameAllocator::new(&boot_info.memory_map)
    };

    allocator::init_heap(&mut mapper, &mut frame_allocator)
        .expect("Failed to initialize Hammerhead Heap Allocator Space");
    vga_buffer::print_something("Heap Allocator: OK [128 KiB]\n");

    vga_buffer::print_something("Enabling hardware interrupts...\n");
    x86_64::instructions::interrupts::enable();

    // Spawn the shell as the only task and hand the CPU to it.
    {
        let mut sched = task::SCHEDULER.lock();
        sched.add_task(task::Task::new(0, shell::run));
    }

    // The boot context idles here; run_yield() switches to the shell on the
    // first call and never returns to this loop while the shell is alive.
    loop {
        task::run_yield();
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga_buffer::print_something("\nKERNEL PANIC: ");
    vga_buffer::print_something(info.message().as_str().unwrap_or("???"));
    loop {
        x86_64::instructions::hlt();
    }
}