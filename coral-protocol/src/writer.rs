use crate::{CONTINUE_BIT, SEGMENT_BITS, packets::play::block::BlockPosition};

pub struct Writer {
    pub data: Vec<u8>,
}

impl Writer {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Write into a buffer that already exists
    pub fn with_buffer(mut data: Vec<u8>) -> Self {
        data.clear();
        Self { data }
    }

    /// Hand the buffer back to be reused by the next [`Self::with_buffer`]
    pub fn into_data(self) -> Vec<u8> {
        self.data
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.data.push(byte);
    }

    pub fn write_varint(&mut self, value: i32) {
        let mut value = value as u32;
        loop {
            if value & !(SEGMENT_BITS as u32) == 0 {
                self.write_byte(value as u8);
                return;
            }
            self.write_byte(((value & SEGMENT_BITS as u32) as u8) | CONTINUE_BIT);
            value >>= 7;
        }
    }

    pub fn write_varint_byte(&mut self, value: i8) {
        let mut value = value as u8;
        loop {
            if value & !SEGMENT_BITS == 0 {
                self.write_byte(value);
                return;
            }
            self.write_byte((value & SEGMENT_BITS) | CONTINUE_BIT);
            value >>= 7;
        }
    }

    pub fn write_bool(&mut self, value: bool) {
        self.write_byte(if value { 0x01 } else { 0x00 });
    }

    pub fn write_u16(&mut self, value: u16) {
        self.write_byte((value >> 8) as u8);
        self.write_byte((value & 0xFF) as u8);
    }

    pub fn write_i32(&mut self, value: i32) {
        self.data.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_i16(&mut self, value: i16) {
        self.data.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_block_position(&mut self, bp: BlockPosition) {
        self.data.extend_from_slice(&bp.to_long().to_be_bytes());
    }

    pub fn write_f32(&mut self, value: f32) {
        self.data.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_f64(&mut self, value: f64) {
        self.data.extend_from_slice(&value.to_be_bytes());
    }

    pub const MAX_STRING_BYTES: usize = 32767;

    pub fn write_string(&mut self, value: &str) {
        let bytes = value.as_bytes();

        let bytes = if bytes.len() > Self::MAX_STRING_BYTES {
            let mut end = Self::MAX_STRING_BYTES;
            while end > 0 && !value.is_char_boundary(end) {
                end -= 1;
            }
            eprintln!(
                "[Protocol] truncating a {}-byte string to {end} bytes",
                bytes.len()
            );
            &bytes[..end]
        } else {
            bytes
        };

        self.write_varint(bytes.len() as i32);
        self.data.extend_from_slice(bytes);
    }

    pub fn write_long(&mut self, value: i64) {
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
impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::Reader;

    fn varint(value: i32) -> Vec<u8> {
        let mut w = Writer::new();
        w.write_varint(value);
        w.data
    }

    #[test]
    fn an_oversized_string_truncates_instead_of_panicking() {
        let mut w = Writer::new();
        w.write_string(&"x".repeat(40_000));

        let mut r = Reader::new(&w.data);
        let got = r.read_string();
        assert_eq!(got.len(), Writer::MAX_STRING_BYTES);
        assert!(r.is_ok());
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn truncation_respects_character_boundaries() {
        let text = "\u{e9ba}".repeat(20_000);
        assert!(!text.is_char_boundary(Writer::MAX_STRING_BYTES));

        let mut w = Writer::new();
        w.write_string(&text);

        let mut r = Reader::new(&w.data);
        let got = r.read_string();
        assert!(r.is_ok(), "the reader rejected the truncated string");
        assert!(got.len() <= Writer::MAX_STRING_BYTES);
        assert!(text.starts_with(&got), "truncation changed the content");
    }

    #[test]
    fn a_string_at_the_limit_is_untouched() {
        let text = "y".repeat(Writer::MAX_STRING_BYTES);
        let mut w = Writer::new();
        w.write_string(&text);
        assert_eq!(Reader::new(&w.data).read_string(), text);
    }

    #[test]
    fn negative_varints_terminate_in_five_bytes() {
        for value in [-1, -128, -1000, i32::MIN, i32::MIN + 1] {
            let bytes = varint(value);
            assert_eq!(bytes.len(), 5, "{value} encoded as {bytes:?}");
        }
    }

    #[test]
    fn varint_widths_match_the_protocol() {
        assert_eq!(varint(0), vec![0x00]);
        assert_eq!(varint(1), vec![0x01]);
        assert_eq!(varint(127).len(), 1);
        assert_eq!(varint(128).len(), 2);
        assert_eq!(varint(16_383).len(), 2);
        assert_eq!(varint(16_384).len(), 3);
        assert_eq!(varint(2_097_151).len(), 3);
        assert_eq!(varint(2_097_152).len(), 4);
        assert_eq!(varint(i32::MAX).len(), 5);
        assert_eq!(varint(-1), vec![0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
        assert_eq!(varint(i32::MIN), vec![0x80, 0x80, 0x80, 0x80, 0x08]);
    }

    #[test]
    fn varints_round_trip_through_the_reader() {
        for value in [
            0,
            1,
            127,
            128,
            255,
            16_383,
            16_384,
            2_097_152,
            i32::MAX,
            -1,
            -128,
            -2_097_152,
            i32::MIN,
        ] {
            let bytes = varint(value);
            let mut r = Reader::new(&bytes);
            assert_eq!(r.read_varint(), value, "{value} did not survive {bytes:?}");
            assert!(r.is_ok(), "{value} left the reader in an error state");
            assert_eq!(r.remaining(), 0, "{value} left trailing bytes");
        }
    }

    #[test]
    fn negative_varint_bytes_terminate_too() {
        for value in [-1i8, -64, i8::MIN] {
            let mut w = Writer::new();
            w.write_varint_byte(value);
            assert!(w.data.len() <= 2, "{value} encoded as {:?}", w.data);
        }
    }
}
