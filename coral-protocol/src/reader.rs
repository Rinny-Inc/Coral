use crate::{
    CONTINUE_BIT, SEGMENT_BITS,
    limits::{MAX_STRING_BYTES, MAX_VARINT_BYTES, MAX_WORLD_COORDINATE},
    packets::play::block::BlockPosition,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    UnexpectedEof,
    NegativeLength(i32),
    LengthTooLarge { len: usize, max: usize },
    VarIntTooLong,
    InvalidUtf8,
    NonFinite,
    OutOfBounds,
    TrailingBytes(usize),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::UnexpectedEof => write!(f, "unexpected end of packet"),
            ReadError::NegativeLength(n) => write!(f, "negative length prefix: {n}"),
            ReadError::LengthTooLarge { len, max } => {
                write!(f, "length {len} exceeds maximum {max}")
            }
            ReadError::VarIntTooLong => write!(f, "varint exceeded 5 bytes"),
            ReadError::InvalidUtf8 => write!(f, "string was not valid UTF-8"),
            ReadError::NonFinite => write!(f, "float was NaN or infinite"),
            ReadError::OutOfBounds => write!(f, "value outside its permitted range"),
            ReadError::TrailingBytes(n) => write!(f, "{n} unconsumed bytes remain"),
        }
    }
}

impl From<ReadError> for std::io::Error {
    fn from(e: ReadError) -> Self {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
    }
}

pub struct Reader<'a> {
    pub data: &'a [u8],
    pub position: usize,
    pub error: Option<ReadError>,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            position: 0,
            error: None,
        }
    }

    pub fn has_remaining(&self) -> bool {
        self.position < self.data.len()
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    fn fail(&mut self, e: ReadError) {
        if self.error.is_none() {
            self.error = Some(e);
        }
    }

    pub fn is_ok(&self) -> bool {
        self.error.is_none()
    }

    pub fn finish(&self) -> Result<(), ReadError> {
        match &self.error {
            Some(e) => Err(e.clone()),
            None => Ok(()),
        }
    }

    pub fn finish_exact(&self) -> Result<(), ReadError> {
        self.finish()?;
        if self.remaining() != 0 {
            return Err(ReadError::TrailingBytes(self.remaining()));
        }
        Ok(())
    }

    pub fn read_byte(&mut self) -> u8 {
        if self.position >= self.data.len() {
            self.fail(ReadError::UnexpectedEof);
            return 0;
        }
        let byte = self.data[self.position];
        self.position += 1;
        byte
    }

    pub fn read_i16(&mut self) -> i16 {
        i16::from_be_bytes([self.read_byte(), self.read_byte()])
    }

    pub fn read_bytes(&mut self, length: usize) -> Vec<u8> {
        if length > self.remaining() {
            self.fail(ReadError::UnexpectedEof);
            self.position = self.data.len();
            return Vec::new();
        }
        let end = self.position.saturating_add(length);
        let bytes = self.data[self.position..end].to_vec();
        self.position = end;
        bytes
    }

    pub fn read_bytes_max(&mut self, max: usize) -> Vec<u8> {
        let Some(len) = self.read_length(max) else {
            return Vec::new();
        };
        self.read_bytes(len)
    }

    pub fn read_remaining_max(&mut self, max: usize) -> Vec<u8> {
        let len = self.remaining();
        if len > max {
            self.fail(ReadError::LengthTooLarge { len, max });
            self.position = self.data.len();
            return Vec::new();
        }
        self.read_bytes(len)
    }

    pub fn read_varint(&mut self) -> i32 {
        let mut value: i32 = 0;
        let mut position = 0;

        for _ in 0..MAX_VARINT_BYTES {
            let current_byte = self.read_byte();
            if self.error.is_some() {
                return 0;
            }
            value |= ((current_byte & SEGMENT_BITS) as i32) << position;

            if (current_byte & CONTINUE_BIT) == 0 {
                return value;
            }

            position += 7;
        }

        self.fail(ReadError::VarIntTooLong);
        0
    }

    pub fn read_length(&mut self, max: usize) -> Option<usize> {
        let raw = self.read_varint();
        if self.error.is_some() {
            return None;
        }
        if raw < 0 {
            self.fail(ReadError::NegativeLength(raw));
            return None;
        }
        let len = raw as usize;
        if len > max {
            self.fail(ReadError::LengthTooLarge { len, max });
            return None;
        }
        Some(len)
    }

    pub fn read_bool(&mut self) -> bool {
        self.read_byte() == 0x01
    }

    pub fn read_float(&mut self) -> f32 {
        let mut bytes = [0u8; 4];
        for b in &mut bytes {
            *b = self.read_byte();
        }
        let v = f32::from_be_bytes(bytes);
        if !v.is_finite() {
            self.fail(ReadError::NonFinite);
            return 0.0;
        }
        v
    }

    pub fn read_double(&mut self) -> f64 {
        let mut bytes = [0u8; 8];
        for b in &mut bytes {
            *b = self.read_byte();
        }
        let v = f64::from_be_bytes(bytes);
        if !v.is_finite() {
            self.fail(ReadError::NonFinite);
            return 0.0;
        }
        v
    }

    pub fn read_coordinate(&mut self) -> f64 {
        let v = self.read_double();
        if v.abs() > MAX_WORLD_COORDINATE {
            self.fail(ReadError::OutOfBounds);
            return 0.0;
        }
        v
    }

    pub fn read_u16(&mut self) -> u16 {
        let high = self.read_byte() as u16;
        let low = self.read_byte() as u16;
        (high << 8) | low
    }

    pub fn read_string_max(&mut self, max: usize) -> String {
        let Some(length) = self.read_length(max) else {
            return String::new();
        };
        if length > self.remaining() {
            self.fail(ReadError::UnexpectedEof);
            self.position = self.data.len();
            return String::new();
        }
        let end = self.position.saturating_add(length);
        let string_bytes = &self.data[self.position..end];
        self.position = end;

        match std::str::from_utf8(string_bytes) {
            Ok(s) => s.to_owned(),
            Err(_) => {
                self.fail(ReadError::InvalidUtf8);
                String::new()
            }
        }
    }

    pub fn read_string(&mut self) -> String {
        self.read_string_max(MAX_STRING_BYTES)
    }

    pub fn read_long(&mut self) -> i64 {
        let mut bytes = [0u8; 8];
        for byte in &mut bytes {
            *byte = self.read_byte();
        }
        i64::from_be_bytes(bytes)
    }

    pub fn read_block_position(&mut self) -> BlockPosition {
        let mut bytes = [0u8; 8];
        for byte in &mut bytes {
            *byte = self.read_byte();
        }
        BlockPosition::from_long(i64::from_be_bytes(bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varint_reference_vectors() {
        let cases: &[(&[u8], i32)] = &[
            (&[0x00], 0),
            (&[0x01], 1),
            (&[0x7f], 127),
            (&[0x80, 0x01], 128),
            (&[0xff, 0x01], 255),
            (&[0xdd, 0xc7, 0x01], 25565),
            (&[0xff, 0xff, 0x7f], 2097151),
            (&[0xff, 0xff, 0xff, 0xff, 0x07], 2147483647),
            (&[0xff, 0xff, 0xff, 0xff, 0x0f], -1),
            (&[0x80, 0x80, 0x80, 0x80, 0x08], -2147483648),
        ];
        for (bytes, expected) in cases {
            let mut r = Reader::new(bytes);
            assert_eq!(r.read_varint(), *expected, "bytes {bytes:?}");
            assert!(r.is_ok());
        }
    }

    #[test]
    fn varint_six_bytes_does_not_panic() {
        let mut r = Reader::new(&[0xff, 0xff, 0xff, 0xff, 0xff, 0x01]);
        r.read_varint();
        assert_eq!(r.finish(), Err(ReadError::VarIntTooLong));
    }

    #[test]
    fn varint_unterminated_is_eof() {
        let mut r = Reader::new(&[0xff, 0xff, 0xff]);
        r.read_varint();
        assert_eq!(r.finish(), Err(ReadError::UnexpectedEof));
    }

    #[test]
    fn read_bytes_beyond_end_does_not_panic() {
        let mut r = Reader::new(&[0x01, 0x02]);
        let out = r.read_bytes(1000);
        assert!(out.is_empty());
        assert_eq!(r.finish(), Err(ReadError::UnexpectedEof));
    }

    #[test]
    fn invalid_utf8_does_not_panic() {
        let mut r = Reader::new(&[0x02, 0xff, 0xfe]);
        let s = r.read_string_max(64);
        assert!(s.is_empty());
        assert_eq!(r.finish(), Err(ReadError::InvalidUtf8));
    }

    #[test]
    fn negative_string_length_rejected() {
        let mut r = Reader::new(&[0xff, 0xff, 0xff, 0xff, 0x0f]);
        r.read_string_max(64);
        assert_eq!(r.finish(), Err(ReadError::NegativeLength(-1)));
    }

    #[test]
    fn oversized_string_rejected_before_allocating() {
        // declares 25565 bytes, supplies none
        let mut r = Reader::new(&[0xdd, 0xc7, 0x01]);
        r.read_string_max(100);
        assert!(matches!(r.finish(), Err(ReadError::LengthTooLarge { .. })));
    }

    #[test]
    fn bool_at_eof_does_not_panic() {
        let mut r = Reader::new(&[]);
        assert!(!r.read_bool());
        assert_eq!(r.finish(), Err(ReadError::UnexpectedEof));
    }

    #[test]
    fn non_finite_floats_rejected() {
        let b = f32::NAN.to_bits().to_be_bytes();
        let mut r = Reader::new(&b);
        r.read_float();
        assert_eq!(r.finish(), Err(ReadError::NonFinite));

        let b = f64::INFINITY.to_bits().to_be_bytes();
        let mut r = Reader::new(&b);
        r.read_double();
        assert_eq!(r.finish(), Err(ReadError::NonFinite));
    }

    #[test]
    fn coordinates_bounded() {
        let b = (1.0e9f64).to_bits().to_be_bytes();
        let mut r = Reader::new(&b);
        r.read_coordinate();
        assert_eq!(r.finish(), Err(ReadError::OutOfBounds));

        let b = (100.5f64).to_bits().to_be_bytes();
        let mut r = Reader::new(&b);
        assert_eq!(r.read_coordinate(), 100.5);
        assert!(r.is_ok());
    }

    #[test]
    fn first_error_is_sticky() {
        let mut r = Reader::new(&[0x02, 0xff, 0xfe]);
        r.read_string_max(64);
        r.read_byte();
        assert_eq!(r.finish(), Err(ReadError::InvalidUtf8));
    }

    #[test]
    fn finish_exact_detects_trailing() {
        let mut r = Reader::new(&[0x01, 0x02, 0x03]);
        r.read_byte();
        assert_eq!(r.finish_exact(), Err(ReadError::TrailingBytes(2)));
    }
}
