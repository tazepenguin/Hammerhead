use core::arch::x86_64::__cpuid;

pub fn run() {
    // Vendor string
    let vendor = __cpuid(0);
    let mut vendor_bytes = [0u8; 12];
    vendor_bytes[0..4].copy_from_slice(&vendor.ebx.to_le_bytes());
    vendor_bytes[4..8].copy_from_slice(&vendor.edx.to_le_bytes());
    vendor_bytes[8..12].copy_from_slice(&vendor.ecx.to_le_bytes());
    let vendor_str = core::str::from_utf8(&vendor_bytes).unwrap_or("Unknown");

    // Brand string
    let mut brand_buf = [0u8; 48];
    let brand = if vendor.eax >= 0x80000004 {
        for (i, leaf) in (0x80000002..=0x80000004).enumerate() {
            let res = __cpuid(leaf);
            brand_buf[i*16 .. i*16+4].copy_from_slice(&res.eax.to_le_bytes());
            brand_buf[i*16+4 .. i*16+8].copy_from_slice(&res.ebx.to_le_bytes());
            brand_buf[i*16+8 .. i*16+12].copy_from_slice(&res.ecx.to_le_bytes());
            brand_buf[i*16+12 .. i*16+16].copy_from_slice(&res.edx.to_le_bytes());
        }
        core::str::from_utf8(&brand_buf).unwrap_or("Unknown").trim_end()
    } else {
        "Brand string not supported"
    };

    crate::vga_buffer::print_something("CPU Vendor: ");
    crate::vga_buffer::print_something(vendor_str);
    crate::vga_buffer::print_something("\nCPU Brand : ");
    crate::vga_buffer::print_something(brand);
    crate::vga_buffer::print_something("\n");
}