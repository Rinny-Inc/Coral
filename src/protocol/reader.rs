use crate::protocol::{SEGMENT_BITS, CONTINUE_BIT};

pub trait Read {
    fn new(data: Vec<u8>) -> Self;
    fn read_byte(&mut self) -> u8;
    fn read_varint(&mut self) -> i32;
    fn read_varlong(&mut self) -> i64;
}

pub struct Reader {
    data: Vec<u8>,
    pub position: usize,
}

impl Read for Reader {
    fn new(data: Vec<u8>) -> Self {
        Self { 
            data, 
            position: 0 
        }
    }

    fn read_byte(&mut self) -> u8 {
        let byte = self.data[self.position];
        self.position += 1;
        byte
    }

    fn read_varint(&mut self) -> i32 {
        let mut value = 0;
        let mut position = 0;

        loop {
            let current_byte = self.read_byte();
            value |= ((current_byte & SEGMENT_BITS) as i32) << position;

            if (current_byte & CONTINUE_BIT) == 0 {
                break;
            }

            position += 7;

            if position >= 32 {
                panic!("VarInt is too big");
            }
        }

        value
    }

    fn read_varlong(&mut self) -> i64 {
        let mut value = 0;
        let mut position = 0;

        loop {
            let current_byte = self.read_byte();
            value |= ((current_byte & SEGMENT_BITS) as i64) << position;

            if (current_byte & CONTINUE_BIT) == 0 {
                break;
            }

            position += 7;

            if position >= 64 {
                panic!("VarLong is too big");
            }
        }

        value
    }
}