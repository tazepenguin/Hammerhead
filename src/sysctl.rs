/// Print a kernel version/build information block.
pub fn version() {
    crate::vga_buffer::print_fmt(format_args!(
        "Hammerhead OS  v{}\n\
         Architecture:  x86_64\n\
         Heap:          128 KiB linked-list allocator @ 0x{:X}\n\
         Timer:         PIT channel 0 @ {} Hz\n\
         Boot:          bootloader 0.9 (BIOS mode)\n",
        env!("CARGO_PKG_VERSION"),
        crate::allocator::HEAP_START,
        crate::interrupts::PIT_HZ,
    ));
}

/// Dump the key CPU control registers and a subset of RFLAGS bits.
pub fn regs() {
    use x86_64::registers::control::{Cr0, Cr2, Cr3, Cr4};
    use x86_64::registers::rflags;

    let cr0  = Cr0::read();
    let cr2  = Cr2::read();
    let (cr3_frame, _) = Cr3::read();
    let cr4  = Cr4::read();
    let rf   = rflags::read();

    // Read raw bits for manual flag decoding — avoids coupling to bitflag
    // constant names that differ across x86_64 crate minor versions.
    let cr0_bits = cr0.bits();
    let cr4_bits = cr4.bits();
    let rf_bits  = rf.bits();

    // CR0 interesting bits
    let pe  = (cr0_bits      ) & 1; // bit  0: Protected Mode Enable
    let wp  = (cr0_bits >> 16) & 1; // bit 16: Write Protect
    let pg  = (cr0_bits >> 31) & 1; // bit 31: Paging

    // CR4 interesting bits
    let pae  = (cr4_bits >> 5) & 1; // bit 5:  Physical Address Extension
    let pge  = (cr4_bits >> 7) & 1; // bit 7:  Page Global Enable
    let smep = (cr4_bits >> 20) & 1; // bit 20: Supervisor Mode Exec Protection

    // RFLAGS interesting bits
    let r_if   = (rf_bits >>  9) & 1; // bit  9: Interrupt enable
    let r_iopl = (rf_bits >> 12) & 3; // bits 12-13: I/O privilege level

    crate::vga_buffer::print_fmt(format_args!(
        "CR0     {:#010x}  PE={pe} WP={wp} PG={pg}\n\
         CR2     {:#018x}  (last page-fault address)\n\
         CR3     {:#018x}  (PML4 frame)\n\
         CR4     {:#010x}  PAE={pae} PGE={pge} SMEP={smep}\n\
         RFLAGS  {:#010x}  IF={r_if} IOPL={r_iopl}\n",
        cr0_bits,
        cr2.as_u64(),
        cr3_frame.start_address().as_u64(),
        cr4_bits,
        rf_bits,
    ));
}