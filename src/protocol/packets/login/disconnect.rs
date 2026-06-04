use crate::protocol::{packets::PacketOut, writer::Writer};

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

impl PacketOut for LoginDisconnect {
    fn encode(&self, writer: &mut Writer) -> std::io::Result<()> {
        writer.write_varint(0x00);
        writer.write_string(&self.reason);
        Ok(())
    }
}
impl PacketOut for PlayDisconnect {
    fn encode(&self, writer: &mut Writer) -> std::io::Result<()> {
        writer.write_varint(0x40);
        writer.write_string(&self.reason);
        Ok(())
    }
}
