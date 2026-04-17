use core::fmt::Write;
use spin::Mutex;
use core::ptr;

const VGA_BUFFER: *mut u8 = 0xb8000 as *mut u8;
const VGA_WIDTH: usize = 80;
const VGA_HEIGHT: usize = 25;
const UART_PORT: u16 = 0x3f8;

static CONSOLE: Mutex<Console> = Mutex::new(Console::new());

struct Console {
    cursor_x: usize,
    cursor_y: usize,
    color: u8,
    buffer: [u8; 4096],
    buf_pos: usize,
}

impl Console {
    const fn new() -> Self {
        Console { cursor_x: 0, cursor_y: 0, color: 0x07, buffer: [0; 4096], buf_pos: 0 }
    }

    fn init_uart(&self) {
        unsafe {
            outb(UART_PORT + 1, 0x00);
            outb(UART_PORT + 3, 0x80);
            outb(UART_PORT + 0, 0x03);
            outb(UART_PORT + 1, 0x00);
            outb(UART_PORT + 3, 0x03);
            outb(UART_PORT + 2, 0xC7);
            outb(UART_PORT + 4, 0x0B);
        }
    }

    fn uart_putc(&self, c: u8) {
        unsafe {
            while (inb(UART_PORT + 5) & 0x20) == 0 {}
            outb(UART_PORT, c);
        }
    }

    fn write_byte(&mut self, byte: u8) {
        self.uart_putc(byte);
        match byte {
            b'\n' => {
                self.newline();
            },
            byte => {
                self.put_char(byte);
            }
        }
    }

    fn put_char(&mut self, c: u8) {
        let index = (self.cursor_y * VGA_WIDTH + self.cursor_x) * 2;
        unsafe {
            ptr::write_volatile(VGA_BUFFER.add(index), c);
            ptr::write_volatile(VGA_BUFFER.add(index + 1), self.color);
        }
        self.cursor_x += 1;
        if self.cursor_x >= VGA_WIDTH {
            self.newline();
        }
    }

    fn newline(&mut self) {
        self.cursor_x = 0;
        self.cursor_y += 1;
        if self.cursor_y >= VGA_HEIGHT {
            self.scroll();
        }
    }

    fn scroll(&mut self) {
        for y in 1..VGA_HEIGHT {
            for x in 0..VGA_WIDTH {
                let src = (y * VGA_WIDTH + x) * 2;
                let dst = ((y - 1) * VGA_WIDTH + x) * 2;
                unsafe {
                    let c = ptr::read_volatile(VGA_BUFFER.add(src));
                    let attr = ptr::read_volatile(VGA_BUFFER.add(src + 1));
                    ptr::write_volatile(VGA_BUFFER.add(dst), c);
                    ptr::write_volatile(VGA_BUFFER.add(dst + 1), attr);
                }
            }
        }
        let last_row = (VGA_HEIGHT - 1) * VGA_WIDTH * 2;
        for i in 0..VGA_WIDTH * 2 {
            unsafe {
                ptr::write_volatile(VGA_BUFFER.add(last_row + i), 0);
            }
        }
        self.cursor_y = VGA_HEIGHT - 1;
    }

    fn clear_screen(&mut self) {
        for y in 0..VGA_HEIGHT {
            for x in 0..VGA_WIDTH {
                let index = (y * VGA_WIDTH + x) * 2;
                unsafe {
                    ptr::write_volatile(VGA_BUFFER.add(index), b' ');
                    ptr::write_volatile(VGA_BUFFER.add(index + 1), self.color);
                }
            }
        }
        self.cursor_x = 0;
        self.cursor_y = 0;
    }

    fn set_cursor(&mut self, x: usize, y: usize) {
        if x < VGA_WIDTH && y < VGA_HEIGHT {
            self.cursor_x = x;
            self.cursor_y = y;
        }
    }

    fn move_cursor(&self) {
        let pos = self.cursor_y * VGA_WIDTH + self.cursor_x;
        unsafe {
            outb(0x3D4, 0x0F);
            outb(0x3D5, pos as u8);
            outb(0x3D4, 0x0E);
            outb(0x3D5, (pos >> 8) as u8);
        }
    }

    fn set_color(&mut self, fg: u8, bg: u8) {
        self.color = (bg << 4) | (fg & 0x0F);
    }

    fn write_str(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x08 | 0x7F => self.backspace(),
                _ => self.write_byte(byte),
            }
        }
    }

    fn backspace(&mut self) {
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
            let index = (self.cursor_y * VGA_WIDTH + self.cursor_x) * 2;
            unsafe {
                ptr::write_volatile(VGA_BUFFER.add(index), b' ');
                ptr::write_volatile(VGA_BUFFER.add(index + 1), self.color);
            }
        } else if self.cursor_y > 0 {
            self.cursor_y -= 1;
            self.cursor_x = VGA_WIDTH - 1;
            let index = (self.cursor_y * VGA_WIDTH + self.cursor_x) * 2;
            unsafe {
                ptr::write_volatile(VGA_BUFFER.add(index), b' ');
                ptr::write_volatile(VGA_BUFFER.add(index + 1), self.color);
            }
        }
    }

    fn write_hex_byte(&mut self, b: u8) {
        self.write_str(&format!("{:02X}", b));
    }

    fn write_hex_word(&mut self, w: u16) {
        self.write_str(&format!("{:04X}", w));
    }

    fn write_hex_dword(&mut self, dw: u32) {
        self.write_str(&format!("{:08X}", dw));
    }

    fn write_hex_qword(&mut self, qw: u64) {
        self.write_str(&format!("{:016X}", qw));
    }

    fn write_dec(&mut self, n: u64) {
        if n == 0 {
            self.write_byte(b'0');
            return;
        }
        let mut num = n;
        let mut buf = [0u8; 20];
        let mut i = 0;
        while num != 0 {
            buf[i] = (num % 10) as u8 + b'0';
            num /= 10;
            i += 1;
        }
        for j in (0..i).rev() {
            self.write_byte(buf[j]);
        }
    }

    fn write_octal(&mut self, n: u64) {
        if n == 0 {
            self.write_byte(b'0');
            return;
        }
        let mut num = n;
        let mut buf = [0u8; 22];
        let mut i = 0;
        while num != 0 {
            buf[i] = (num & 7) as u8 + b'0';
            num >>= 3;
            i += 1;
        }
        for j in (0..i).rev() {
            self.write_byte(buf[j]);
        }
    }

    fn write_binary(&mut self, n: u64) {
        if n == 0 {
            self.write_byte(b'0');
            return;
        }
        let mut num = n;
        let mut buf = [0u8; 64];
        let mut i = 0;
        while num != 0 {
            buf[i] = (num & 1) as u8 + b'0';
            num >>= 1;
            i += 1;
        }
        for j in (0..i).rev() {
            self.write_byte(buf[j]);
        }
    }

    fn flush_buffer(&mut self) {
        for i in 0..self.buf_pos {
            self.write_byte(self.buffer[i]);
        }
        self.buf_pos = 0;
    }

    fn buffer_write(&mut self, c: u8) {
        if self.buf_pos < self.buffer.len() {
            self.buffer[self.buf_pos] = c;
            self.buf_pos += 1;
        } else {
            self.flush_buffer();
            self.buffer[0] = c;
            self.buf_pos = 1;
        }
    }

    fn enable_cursor(&self) {
        unsafe {
            outb(0x3D4, 0x0A);
            let mut c = inb(0x3D5) & 0xE0;
            outb(0x3D5, c);
            
            outb(0x3D4, 0x0B);
            c = inb(0x3D5) & 0xE0;
            outb(0x3D5, c | 0x0F);
        }
    }

    fn disable_cursor(&self) {
        unsafe {
            outb(0x3D4, 0x0A);
            let c = inb(0x3D5) | 0x20;
            outb(0x3D5, c);
        }
    }

    fn write_char_with_attr(&mut self, c: u8, attr: u8) {
        let index = (self.cursor_y * VGA_WIDTH + self.cursor_x) * 2;
        unsafe {
            ptr::write_volatile(VGA_BUFFER.add(index), c);
            ptr::write_volatile(VGA_BUFFER.add(index + 1), attr);
        }
        self.cursor_x += 1;
        if self.cursor_x >= VGA_WIDTH {
            self.newline();
        }
    }
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write_str(s);
        Ok(())
    }
}

fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!(
            "out dx, al",
            in("dx") port,
            in("al") value,
            options(nostack, preserves_flags)
        );
    }
}

fn inb(port: u16) -> u8 {
    let ret: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            out("al") ret,
            in("dx") port,
            options(nostack, preserves_flags)
        );
    }
    ret
}

pub fn print(args: core::fmt::Arguments) {
    CONSOLE.lock().write_fmt(args).unwrap();
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::console::print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => {
        $crate::print!("{}\n", format_args!($($arg)*));
    };
}

