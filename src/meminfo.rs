use bootloader::bootinfo::MemoryRegionType;

pub fn run() {
    if let Some(info) = crate::BOOT_INFO.get() {
        crate::vga_buffer::print_something("Memory map (usable regions):\n");
        let mut total_usable = 0u64;
        for region in info.memory_map.iter() {
            if region.region_type == MemoryRegionType::Usable {
                let start = region.range.start_addr();
                let end = region.range.end_addr();
                let size = end - start;
                total_usable += size;
                crate::vga_buffer::print_fmt(format_args!(
                    "  0x{:016X} - 0x{:016X}  ({:>6} KB)\n",
                    start, end, size / 1024
                ));
            }
        }
        crate::vga_buffer::print_fmt(format_args!(
            "Total usable memory: {} MB\n",
            total_usable / 1024 / 1024
        ));
    } else {
        crate::vga_buffer::print_something("Boot info not available.\n");
    }
}