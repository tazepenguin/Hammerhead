use core::arch::x86_64::__cpuid;

pub fn run() {
    let max_extended = __cpuid(0x80000000).eax;
    if max_extended >= 0x80000004 {
        let mut brand_string = [0u8; 48];
        for i in 0..3 {
            let res = __cpuid(0x80000002 + i);
            let offset = (i as usize) * 16;
            brand_string[offset..offset + 4].copy_from_slice(&res.eax.to_ne_bytes());
            brand_string[offset + 4..offset + 8].copy_from_slice(&res.ebx.to_ne_bytes());
            brand_string[offset + 8..offset + 12].copy_from_slice(&res.ecx.to_ne_bytes());
            brand_string[offset + 12..offset + 16].copy_from_slice(&res.edx.to_ne_bytes());
        }
        let s = core::str::from_utf8(&brand_string).unwrap_or("Invalid brand string");
        crate::vga_buffer::print_something(s.trim());
        crate::vga_buffer::print_something("\n");
    } else {
        crate::vga_buffer::print_something("Brand string not supported\n");
    }
}