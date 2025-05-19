use crate::protocol::{SEGMENT_BITS, CONTINUE_BIT};

pub struct Writer {
    pub data: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self { 
            data: Vec::new()
        }
    }

    fn write_byte(&mut self, byte: u8) {
        self.data.push(byte);
    }

    pub fn write_varint(&mut self, mut value: i32) {
        while (value & !(SEGMENT_BITS as i32)) != 0 {
            self.write_byte(((value & SEGMENT_BITS as i32) as u8) | CONTINUE_BIT);
            value >>= 7;
        }
        self.write_byte(value as u8);
    }

    pub fn write_varlong(&mut self, mut value: i64) {
        while (value & !(SEGMENT_BITS as i64)) != 0 {
            self.write_byte(((value & SEGMENT_BITS as i64) as u8) | CONTINUE_BIT);
            value >>= 7;
        }
        self.write_byte(value as u8);
    }
}
