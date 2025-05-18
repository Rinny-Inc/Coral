use crate::protocol::{SEGMENT_BITS, CONTINUE_BIT};

pub trait Write {
    fn new() -> Self;
    fn write_byte(&mut self, byte: u8);
    fn write_varint(&mut self, value: i32);
    fn write_varlong(&mut self, value: i64);
}

pub struct Writer {
    pub data: Vec<u8>,
}

impl Write for Writer {
    fn new() -> Self {
        Self { 
            data: Vec::new()
        }
    }

    fn write_byte(&mut self, byte: u8) {
        self.data.push(byte);
    }

    fn write_varint(&mut self, mut value: i32) {
        while (value & !(SEGMENT_BITS as i32)) != 0 {
            self.write_byte(((value & SEGMENT_BITS as i32) as u8) | CONTINUE_BIT);
            value >>= 7;
        }
        self.write_byte(value as u8);
    }

    fn write_varlong(&mut self, mut value: i64) {
        while (value & !(SEGMENT_BITS as i64)) != 0 {
            self.write_byte(((value & SEGMENT_BITS as i64) as u8) | CONTINUE_BIT);
            value >>= 7;
        }
        self.write_byte(value as u8);
    }
}
