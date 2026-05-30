pub mod auth;
pub mod encryption;
pub mod packets;
pub mod reader;
pub mod writer;

pub const SEGMENT_BITS: u8 = 0x7F;
pub const CONTINUE_BIT: u8 = 0x80;
