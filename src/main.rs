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
mod pci;
mod sysctl;

static BOOT_INFO: Once<&'static BootInfo> = Once::new();

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    BOOT_INFO.call_once(|| boot_info);

    vga_buffer::init();

    gdt::init();
    interrupts::init_idt();
    vga_buffer::print_something("GDT & IDT: OK\n");

    unsafe { interrupts::PICS.lock().initialize(); }
    vga_buffer::print_something("PIC: OK\n");

    // Programme the PIT to a known 100 Hz before enabling interrupts so that
    // TICK_COUNT is accurate from the very first tick.
    interrupts::init_pit(100);
    vga_buffer::print_fmt(format_args!("PIT: {} Hz\n", interrupts::PIT_HZ));

    let phys_offset = x86_64::VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_offset) };
    let mut frame_alloc = unsafe { memory::BootInfoFrameAllocator::new(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_alloc)
        .expect("heap init failed");
    vga_buffer::print_something("Heap: OK [128 KiB]\n");

    vga_buffer::print_something("Enabling interrupts...\n");
    x86_64::instructions::interrupts::enable();

    // Spawn the shell as the sole scheduled task and hand the CPU to it.
    {
        let mut sched = task::SCHEDULER.lock();
        sched.add_task(task::Task::new(0, shell::run));
    }

    // Boot context idles here; switch_context in run_yield() jumps to the
    // shell on the first call and won't return while the shell is alive.
    loop {
        task::run_yield();
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    vga_buffer::print_something("\n\n*** KERNEL PANIC ***\n");
    vga_buffer::print_something(info.message().as_str().unwrap_or("(no message)"));
    vga_buffer::print_something("\n");
    loop { x86_64::instructions::hlt(); }
}