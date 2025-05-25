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

    pub fn write_varshort(&mut self, mut value: i16) {
        while (value & !(SEGMENT_BITS as i16)) != 0 {
            self.write_byte(((value & SEGMENT_BITS as i16) as u8) | CONTINUE_BIT);
            value >>= 7;
        }
        self.write_byte(value as u8);
    }

    pub fn write_varbyte(&mut self, mut value: i8) {
        while (value & !SEGMENT_BITS as i8) != 0 {
            self.write_byte(((value & SEGMENT_BITS as i8) as u8) | CONTINUE_BIT);
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

    pub fn write_bool(&mut self, value: bool) {
        self.write_byte(if value { 0x01 } else { 0x00 });
    }

    pub fn write_u16(&mut self, value: u16) {
        self.write_byte((value >> 8) as u8);
        self.write_byte((value & 0xFF) as u8);
    }

    pub fn write_string(&mut self, value: &str) {
        let bytes = value.as_bytes();

        if bytes.len() > 32767 {
            panic!("String too long: {} bytes", bytes.len());
        }

        self.write_varint(bytes.len() as i32);
        self.data.extend_from_slice(bytes);
    }

    fn write_long(&mut self, value: i64) {
        self.data.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_uuid(&mut self, uuid: &uuid::Uuid) {
        let bytes = uuid.as_bytes();
        let msb = i64::from_be_bytes(bytes[0..8].try_into().unwrap());
        let lsb = i64::from_be_bytes(bytes[8..16].try_into().unwrap());
        self.write_long(msb);
        self.write_long(lsb);
    }
}
