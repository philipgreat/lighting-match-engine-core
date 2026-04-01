use core::fmt::{self, Write};
use core::ptr::addr_of_mut;

const COM1: u16 = 0x3f8;

static mut SERIAL: SerialPort = SerialPort::new(COM1);

pub struct SerialPort {
    base: u16,
}

impl SerialPort {
    pub const fn new(base: u16) -> Self {
        Self { base }
    }

    pub fn init(&mut self) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            outb(self.base + 1, 0x00);
            outb(self.base + 3, 0x80);
            outb(self.base, 0x03);
            outb(self.base + 1, 0x00);
            outb(self.base + 3, 0x03);
            outb(self.base + 2, 0xc7);
            outb(self.base + 4, 0x0b);
        }
    }

    pub fn write_line(&mut self, text: &str) {
        for byte in text.bytes() {
            self.write_byte(byte);
        }
        self.write_byte(b'\n');
    }

    pub fn write_u64(&mut self, label: &str, value: u64) {
        let mut buf = [0u8; 20];
        let text = u64_to_ascii(value, &mut buf);
        self.write_line(label);
        self.write_line(text);
    }

    pub fn write_fmt_line(&mut self, args: fmt::Arguments<'_>) {
        let mut adapter = SerialAdapter { serial: self };
        let _ = adapter.write_fmt(args);
        self.write_byte(b'\n');
    }

    fn write_byte(&mut self, byte: u8) {
        #[cfg(target_arch = "x86_64")]
        unsafe {
            if byte == b'\n' {
                self.wait_for_tx();
                outb(self.base, b'\r');
            }
            self.wait_for_tx();
            outb(self.base, byte);
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn wait_for_tx(&self) {
        unsafe {
            while (inb(self.base + 5) & 0x20) == 0 {}
        }
    }
}

pub fn serial() -> &'static mut SerialPort {
    unsafe { &mut *addr_of_mut!(SERIAL) }
}

struct SerialAdapter<'a> {
    serial: &'a mut SerialPort,
}

impl Write for SerialAdapter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.serial.write_byte(byte);
        }
        Ok(())
    }
}

#[cfg(target_arch = "x86_64")]
unsafe fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
    }
}

#[cfg(target_arch = "x86_64")]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    unsafe {
        core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    }
    value
}

fn u64_to_ascii(mut value: u64, buf: &mut [u8; 20]) -> &str {
    if value == 0 {
        buf[19] = b'0';
        return core::str::from_utf8(&buf[19..20]).unwrap_or("0");
    }

    let mut index = buf.len();
    while value > 0 {
        index -= 1;
        buf[index] = b'0' + (value % 10) as u8;
        value /= 10;
    }

    core::str::from_utf8(&buf[index..]).unwrap_or("")
}
