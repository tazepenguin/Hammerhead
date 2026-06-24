use x86_64::instructions::port::Port;

fn read_cmos(reg: u8) -> u8 {
    let mut addr = Port::new(0x70);
    let mut data = Port::new(0x71);
    unsafe {
        addr.write(reg);
        data.read()
    }
}

fn bcd_to_bin(bcd: u8) -> u8 {
    (bcd & 0x0F) + ((bcd >> 4) * 10)
}

pub fn run() {
    // Wait for RTC to be ready
    while read_cmos(0x0A) & 0x80 != 0 {}

    let mut second = read_cmos(0x00);
    let mut minute = read_cmos(0x02);
    let mut hour   = read_cmos(0x04);
    let mut day    = read_cmos(0x07);
    let mut month  = read_cmos(0x08);
    let mut year   = read_cmos(0x09);

    let reg_b = read_cmos(0x0B);
    if reg_b & 0x04 == 0 {
        // BCD format
        second = bcd_to_bin(second);
        minute = bcd_to_bin(minute);
        hour   = bcd_to_bin(hour);
        day    = bcd_to_bin(day);
        month  = bcd_to_bin(month);
        year   = bcd_to_bin(year);
    }

    // 12-hour mode?
    if reg_b & 0x02 == 0 {
        let pm = (hour & 0x80) != 0;
        hour = hour & 0x7F;
        if pm {
            hour = (hour + 12) % 24;
        }
    }

    let year_full = 2000 + year as u16;

    crate::vga_buffer::print_fmt(format_args!(
        "{:02}/{:02}/{:04} {:02}:{:02}:{:02}\n",
        month, day, year_full, hour, minute, second
    ));
}