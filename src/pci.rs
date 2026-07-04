// PCI bus enumeration via the legacy I/O-port mechanism.
//
// The host bridge exposes two 32-bit I/O registers:
//   0xCF8  CONFIG_ADDRESS  — write the target address here
//   0xCFC  CONFIG_DATA     — then read/write data here
//
// Address format (32 bits):
//   bit 31   : Enable bit (must be 1 for a valid config cycle)
//   bits 23:16: Bus number
//   bits 15:11: Device (slot) number
//   bits 10:8 : Function number
//   bits  7:2 : Register offset (DWORD-aligned)
//   bits  1:0 : Must be 0

use x86_64::instructions::port::Port;

// ── Config-space I/O ─────────────────────────────────────────────────────────

/// Read a 32-bit dword from PCI configuration space.
fn cfg_read32(bus: u8, slot: u8, func: u8, offset: u8) -> u32 {
    let addr: u32 = (1 << 31)
        | ((bus  as u32) << 16)
        | ((slot as u32) << 11)
        | ((func as u32) <<  8)
        | ((offset & 0xFC) as u32);
    unsafe {
        Port::<u32>::new(0xCF8).write(addr);
        Port::<u32>::new(0xCFC).read()
    }
}

// ── Class name table ─────────────────────────────────────────────────────────

/// Return a short human-readable name for a PCI (class, subclass) pair.
fn class_name(class: u8, sub: u8) -> &'static str {
    match (class, sub) {
        (0x00, 0x00) => "Unclassified (non-VGA)",
        (0x00, 0x01) => "Unclassified (VGA)",
        (0x01, 0x00) => "SCSI Controller",
        (0x01, 0x01) => "IDE Controller",
        (0x01, 0x05) => "ATA Controller",
        (0x01, 0x06) => "SATA Controller (AHCI)",
        (0x01, 0x08) => "NVMe Controller",
        (0x01, _)    => "Mass Storage",
        (0x02, 0x00) => "Ethernet",
        (0x02, 0x80) => "Other Network",
        (0x02, _)    => "Network",
        (0x03, 0x00) => "VGA Compatible Display",
        (0x03, 0x01) => "XGA Display",
        (0x03, _)    => "Display",
        (0x04, 0x00) => "Video",
        (0x04, 0x01) => "Audio (legacy)",
        (0x04, 0x03) => "HD Audio",
        (0x04, _)    => "Multimedia",
        (0x05, _)    => "Memory Controller",
        (0x06, 0x00) => "Host Bridge",
        (0x06, 0x01) => "ISA Bridge",
        (0x06, 0x04) => "PCI-to-PCI Bridge",
        (0x06, 0x09) => "PCI-to-PCI Bridge (Semi-Trans)",
        (0x06, _)    => "Bridge",
        (0x07, 0x00) => "Serial (16450/16550)",
        (0x07, 0x01) => "Parallel Port",
        (0x07, _)    => "Communication Controller",
        (0x08, 0x00) => "PIC",
        (0x08, 0x01) => "DMA Controller",
        (0x08, 0x02) => "Timer",
        (0x08, 0x03) => "RTC",
        (0x08, _)    => "System Peripheral",
        (0x09, 0x00) => "Keyboard",
        (0x09, 0x02) => "Mouse",
        (0x09, _)    => "Input Device",
        (0x0B, 0x20) => "x86 Processor",
        (0x0B, _)    => "Processor",
        (0x0C, 0x00) => "FireWire (IEEE 1394)",
        (0x0C, 0x03) => "USB Controller",
        (0x0C, 0x05) => "SMBus Controller",
        (0x0C, _)    => "Serial Bus Controller",
        (0x0D, 0x00) => "iRDA",
        (0x0D, 0x11) => "Bluetooth",
        (0x0D, _)    => "Wireless Controller",
        (0x10, _)    => "Encryption Controller",
        (0x11, _)    => "Signal Processing",
        (0xFF, _)    => "Unassigned",
        _            => "Unknown",
    }
}

// ── Public scan entry point ───────────────────────────────────────────────────

/// Enumerate PCI devices and print a summary table.
///
/// Scans buses 0–15 (512 function-0 probes); bridges to higher buses are
/// noted but not recursed into.  Full multi-bus / multi-function traversal
/// can be added once a heap-backed device list exists.
pub fn run() {
    let mut found = 0u32;

    crate::vga_buffer::print_something(
        "Bus:Slot  Vendor:Dev  Class\n\
         --------  ----------  -----\n"
    );

    for bus in 0u8..=15 {
        for slot in 0u8..32 {
            let id_reg  = cfg_read32(bus, slot, 0, 0x00);
            let vendor  = (id_reg & 0x0000_FFFF) as u16;
            if vendor == 0xFFFF { continue; } // slot empty

            let device  = ((id_reg >> 16) & 0xFFFF) as u16;
            let rev_reg = cfg_read32(bus, slot, 0, 0x08);
            let class   = ((rev_reg >> 24) & 0xFF) as u8;
            let subclass= ((rev_reg >> 16) & 0xFF) as u8;

            crate::vga_buffer::print_fmt(format_args!(
                "  {:02X}:{:02X}   {:04X}:{:04X}   {}\n",
                bus, slot, vendor, device, class_name(class, subclass)
            ));
            found += 1;
        }
    }

    if found == 0 {
        crate::vga_buffer::print_something("  No PCI devices found on buses 0-15.\n");
    } else {
        crate::vga_buffer::print_fmt(format_args!("{} device(s).\n", found));
    }
}