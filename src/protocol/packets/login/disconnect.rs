use std::io::Error;

use crate::protocol::{packets::Packet, writer::Writer};

#[derive(Debug)]
pub struct LoginDisconnect {
    reason: String,
}

#[derive(Debug)]
pub struct PlayDisconnect {
    reason: String,
}

impl LoginDisconnect {
    pub fn new(reason: &str) -> Self {
        Self {
            reason: format!("{{\"text\":\"{}\"}}", reason),
        }
    }
}
impl PlayDisconnect {
    pub fn new(reason: &str) -> Self {
        Self {
            reason: format!("{{\"text\":\"{}\"}}", reason),
        }
    }
}

impl Packet for LoginDisconnect {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self> {
        Err(Error::other("Unexpected call"))
    }

    fn encode(&self, writer: &mut Writer) -> std::io::Result<()> {
        writer.write_varint(0x00);
        writer.write_string(&self.reason);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl Packet for PlayDisconnect {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self> {
        Err(Error::other("Unexpected call"))
    }

    fn encode(&self, writer: &mut Writer) -> std::io::Result<()> {
        writer.write_varint(0x40);
        writer.write_string(&self.reason);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
