pub mod reader;
pub mod writer;
pub mod packets;

pub const SEGMENT_BITS: u8 = 0x7F;
pub const CONTINUE_BIT: u8 = 0x80;
