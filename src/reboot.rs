use x86_64::instructions::port::Port;

pub fn reboot() -> ! {
    let mut port: Port<u8> = Port::new(0x64);
    unsafe {
        loop {
            while (port.read() & 0x02) != 0 {}  // wait for input buffer empty
            port.write(0xFE);                   // pulse reset line
        }
    }
}

pub fn halt() -> ! {
    crate::vga_buffer::print_something("\nSystem halted. You can now turn off the computer.\n");
    loop {
        x86_64::instructions::hlt();
    }
}