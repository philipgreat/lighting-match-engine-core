use core::fmt::{self, Write};
use core::ptr::{addr_of_mut, read_volatile, write_volatile};

const VGA_BUFFER: *mut u16 = 0xb8000 as *mut u16;
const WIDTH: usize = 80;
const HEIGHT: usize = 25;
const COLOR: u8 = 0x0f;

static mut WRITER: Writer = Writer::new();

pub struct Writer {
    column: usize,
    row: usize,
}

impl Writer {
    pub const fn new() -> Self {
        Self {
            column: 0,
            row: 0,
        }
    }

    pub fn clear(&mut self) {
        for row in 0..HEIGHT {
            for col in 0..WIDTH {
                self.write_cell(row, col, b' ');
            }
        }
        self.column = 0;
        self.row = 0;
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

    pub fn write_u32(&mut self, label: &str, value: u32) {
        self.write_u64(label, value as u64);
    }

    pub fn write_fmt_line(&mut self, args: fmt::Arguments<'_>) {
        let mut adapter = WriterAdapter { writer: self };
        let _ = adapter.write_fmt(args);
        self.write_byte(b'\n');
    }

    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            value => {
                if self.column >= WIDTH {
                    self.new_line();
                }

                self.write_cell(self.row, self.column, value);
                self.column += 1;
            }
        }
    }

    fn new_line(&mut self) {
        self.column = 0;
        if self.row + 1 < HEIGHT {
            self.row += 1;
            return;
        }

        for y in 1..HEIGHT {
            for x in 0..WIDTH {
                let from = offset(y, x);
                let to = offset(y - 1, x);
                let value = unsafe { read_volatile(VGA_BUFFER.add(from)) };
                unsafe {
                    write_volatile(VGA_BUFFER.add(to), value);
                }
            }
        }
        for x in 0..WIDTH {
            self.write_cell(HEIGHT - 1, x, b' ');
        }
    }

    fn write_cell(&mut self, row: usize, col: usize, byte: u8) {
        let value = ((COLOR as u16) << 8) | byte as u16;
        unsafe {
            write_volatile(VGA_BUFFER.add(offset(row, col)), value);
        }
    }
}

pub fn writer() -> &'static mut Writer {
    unsafe { &mut *addr_of_mut!(WRITER) }
}

struct WriterAdapter<'a> {
    writer: &'a mut Writer,
}

impl Write for WriterAdapter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.writer.write_byte(byte);
        }
        Ok(())
    }
}

fn offset(row: usize, col: usize) -> usize {
    row * WIDTH + col
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
