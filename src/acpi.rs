use core::ptr::NonNull;
use ::acpi::{AcpiTables, PhysicalMapping, AcpiHandler};
use ::acpi::fadt::Fadt;
use x86_64::instructions::port::Port;

// ── ACPI handler ─────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct HammerheadAcpiHandler {
    offset: u64, // bootloader physical_memory_offset
}

impl AcpiHandler for HammerheadAcpiHandler {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        let virt = physical_address as u64 + self.offset;
        PhysicalMapping::new(
            physical_address,
            NonNull::new(virt as *mut T).unwrap(),
            size,
            size,
            self.clone(),
        )
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {}
}

// ── AML helpers ──────────────────────────────────────────────────────────────

/// Read a small AML integer from `aml[*p..]` and advance `*p` past it.
///
/// Only the encodings that `_S5_` packages realistically use are handled:
///
/// | Opcode | Encoding       | Value        |
/// |--------|----------------|--------------|
/// | 0x00   | ZeroOp         | 0            |
/// | 0x01   | OneOp          | 1            |
/// | 0x0A   | BytePrefix + b | b as u16     |
/// | 0x0B   | WordPrefix + w | w (LE u16)   |
fn read_aml_int(aml: &[u8], p: &mut usize) -> Option<u16> {
    if *p >= aml.len() { return None; }
    match aml[*p] {
        0x00 => { *p += 1; Some(0) }
        0x01 => { *p += 1; Some(1) }
        0x0A => {
            if *p + 1 >= aml.len() { return None; }
            let v = aml[*p + 1] as u16;
            *p += 2;
            Some(v)
        }
        0x0B => {
            if *p + 2 >= aml.len() { return None; }
            let v = u16::from_le_bytes([aml[*p + 1], aml[*p + 2]]);
            *p += 3;
            Some(v)
        }
        _ => None,
    }
}

/// Scan raw AML bytecode for `Name (_S5_, Package { slp_typ_a, slp_typ_b })`
/// and return `(SLP_TYPa, SLP_TYPb)`.
///
/// Wire format being searched:
/// ```text
/// 08          DefName opcode
/// 5F 53 35 5F "_S5_"
/// 12          PackageOp
/// xx [xx..]   PkgLength (byte 0 bits[7:6] = how many extra bytes follow)
/// xx          NumElements
/// <int>       SLP_TYPa  — encoded as ZeroOp / OneOp / BytePrefix / WordPrefix
/// <int>       SLP_TYPb
/// ```
unsafe fn find_s5_in_aml(aml: &[u8]) -> Option<(u16, u16)> {
    // Minimum remaining bytes needed after the needle to be worth checking.
    const MIN_TAIL: usize = 5; // PackageOp + PkgLen + NumEl + two ZeroOps
    let needle = b"\x08_S5_";

    let mut i = 0;
    while i + needle.len() + MIN_TAIL < aml.len() {
        if !aml[i..].starts_with(needle) {
            i += 1;
            continue;
        }

        let mut p = i + needle.len();

        // Must be followed by a Package opcode.
        if p >= aml.len() || aml[p] != 0x12 { i += 1; continue; }
        p += 1;

        // Skip PkgLength: the top two bits of the first byte encode how many
        // additional bytes extend the length field (0..=3 extra bytes).
        if p >= aml.len() { break; }
        let extra = (aml[p] >> 6) as usize;
        p += 1 + extra;

        if p >= aml.len() { break; }
        p += 1; // skip NumElements

        let slp_a = read_aml_int(aml, &mut p)?;
        let slp_b = read_aml_int(aml, &mut p).unwrap_or(0);
        return Some((slp_a, slp_b));
    }
    None
}

// ── DSDT access ──────────────────────────────────────────────────────────────

/// Read the DSDT physical address from the mapped FADT and scan the DSDT's
/// AML bytecode for the `\_S5_` object.
///
/// # How the FADT offset is determined
///
/// `acpi::AcpiTables::find_table::<Fadt>()` may map the table with or without
/// the 36-byte SDT header depending on the crate version.  We auto-detect by
/// checking whether the first four bytes spell `"FACP"` (the FADT signature).
///
/// * **Header present** (first 4 bytes = `"FACP"`):
///   `firmware_ctrl` (u32) is at offset 36, `dsdt` (u32) at offset 40.
/// * **Body only** (first 4 bytes = `firmware_ctrl` data):
///   `firmware_ctrl` (u32) is at offset 0, `dsdt` (u32) at offset 4.
///
/// The DSDT SDT header is 36 bytes; AML starts immediately after it.
unsafe fn s5_from_fadt(fadt: &Fadt, phys_offset: u64) -> Option<(u16, u16)> {
    let p = fadt as *const Fadt as *const u8;

    let has_header = &*p.cast::<[u8; 4]>() == b"FACP";
    let dsdt_offset = if has_header { 40usize } else { 4usize };

    let dsdt_phys = u32::from_le_bytes([
        *p.add(dsdt_offset),
        *p.add(dsdt_offset + 1),
        *p.add(dsdt_offset + 2),
        *p.add(dsdt_offset + 3),
    ]) as u64;

    if dsdt_phys == 0 { return None; }

    // The bootloader linearly maps all physical memory; we can reach the DSDT
    // directly without an extra map_physical_region call.
    let dsdt_virt = dsdt_phys + phys_offset;

    // SDT header: signature(4) + length(4) + ...  length is at bytes 4-7.
    let dsdt_len = u32::from_le_bytes([
        *(dsdt_virt as *const u8).add(4),
        *(dsdt_virt as *const u8).add(5),
        *(dsdt_virt as *const u8).add(6),
        *(dsdt_virt as *const u8).add(7),
    ]) as usize;

    let aml_len = dsdt_len.saturating_sub(36);
    if aml_len == 0 { return None; }

    let aml = core::slice::from_raw_parts((dsdt_virt + 36) as *const u8, aml_len);
    find_s5_in_aml(aml)
}

// ── Public shutdown entry point ───────────────────────────────────────────────

pub fn shutdown() -> ! {
    if let Some(boot_info) = crate::BOOT_INFO.get() {
        let phys_offset = boot_info.physical_memory_offset;
        let handler = HammerheadAcpiHandler { offset: phys_offset };

        if let Ok(tables) = unsafe { AcpiTables::search_for_rsdp_bios(handler) } {
            if let Ok(fadt) = tables.find_table::<Fadt>() {
                // Read the real S5-sleep type values from the firmware DSDT.
                // These differ between platforms; hardcoding any particular
                // value (e.g. 0x5 or 0x7) causes shutdown to silently fail on
                // machines where the firmware chose different values.
                let (slp_typ_a, slp_typ_b) =
                    unsafe { s5_from_fadt(&*fadt, phys_offset).unwrap_or((0, 0)) };

                const SLP_EN: u16 = 1 << 13;

                // PM1a and PM1b receive different SLP_TYP values (SLP_TYPa /
                // SLP_TYPb from the _S5_ package).  For most real hardware and
                // for QEMU they are both 0, but the spec allows them to differ.
                if let Ok(pm1a_block) = fadt.pm1a_control_block() {
                    let mut pm1a: Port<u16> = Port::new(pm1a_block.address as u16);
                    unsafe { pm1a.write((slp_typ_a << 10) | SLP_EN); }

                    if let Ok(Some(pm1b_block)) = fadt.pm1b_control_block() {
                        let mut pm1b: Port<u16> = Port::new(pm1b_block.address as u16);
                        unsafe { pm1b.write((slp_typ_b << 10) | SLP_EN); }
                    }
                }
            }
        }
    }

    // If we reach here the write didn't trigger a power-off.
    crate::vga_buffer::print_something("\nACPI shutdown failed — halting.\n");
    loop { x86_64::instructions::hlt(); }
}