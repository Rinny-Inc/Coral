use crate::protocol::{CONTINUE_BIT, SEGMENT_BITS};

pub struct Reader {
    pub data: Vec<u8>,
    pub position: usize,
}

impl Reader {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, position: 0 }
    }

    pub fn has_remaining(&self) -> bool {
        self.position < self.data.len()
    }

    pub fn remaining(&self) -> usize {
        self.data.len() - self.position
    }

    fn read_byte(&mut self) -> u8 {
        if self.position >= self.data.len() {
            panic!(
                "Attempted to read byte at position {}, but data length is {}",
                self.position,
                self.data.len()
            );
        }
        let byte = self.data[self.position];
        self.position += 1;
        byte
    }

    pub fn read_varint(&mut self) -> i32 {
        let mut value = 0;
        let mut position = 0;
        let mut bytes_read = Vec::new();
        let mut current_byte = self.read_byte();
        bytes_read.push(current_byte);

        while (current_byte & CONTINUE_BIT) != 0 {
            value |= ((current_byte & SEGMENT_BITS) as i32) << position;
            position += 7;

            if position >= 32 {
                panic!("VarInt is too big");
            }

            current_byte = self.read_byte();
            bytes_read.push(current_byte);
        }

        value |= ((current_byte & SEGMENT_BITS) as i32) << position;

        println!("read_varint -> value: {}, bytes: {:?}", value, bytes_read);

        value
    }

    pub fn read_varlong(&mut self) -> i64 {
        let mut value = 0;
        let mut position = 0;
        let mut current_byte = self.read_byte();

        while (current_byte & CONTINUE_BIT) != 0 {
            value |= ((current_byte & SEGMENT_BITS) as i64) << position;
            position += 7;

            if position >= 64 {
                panic!("VarLong is too big");
            }

            current_byte = self.read_byte();
        }

        value |= ((current_byte & SEGMENT_BITS) as i64) << position;
        value
    }

    pub fn read_bool(&mut self) -> bool {
        match self.read_byte() {
            0x00 => false,
            0x01 => true,
            other => panic!("Invalid boolean value: {}", other),
        }
    }

    pub fn read_u16(&mut self) -> u16 {
        let high = self.read_byte() as u16;
        let low = self.read_byte() as u16;
        (high << 8) | low
    }

    pub fn read_string(&mut self) -> String {
        let length = self.read_varint() as usize;
        let end = self.position + length;

        if end > self.data.len() {
            panic!("String length goes beyond buffer");
        }

        let string_bytes = &self.data[self.position..end];
        self.position = end;

        match String::from_utf8(string_bytes.to_vec()) {
            Ok(s) => s,
            Err(e) => panic!("Invalid UTF-8 string: {}", e),
        }
    }

    pub fn read_long(&mut self) -> i64 {
        let mut bytes = [0u8; 8];
        for byte in &mut bytes {
            *byte = self.read_byte();
        }
        i64::from_be_bytes(bytes)
    }

    pub fn read_uuid(&mut self) -> uuid::Uuid {
        let msb = self.read_long() as u64;
        let lsb = self.read_long() as u64;

        let mut bytes = [0u8; 16];
        bytes[..8].copy_from_slice(&msb.to_be_bytes());
        bytes[8..].copy_from_slice(&lsb.to_be_bytes());

        uuid::Uuid::from_bytes(bytes)
    }
}
