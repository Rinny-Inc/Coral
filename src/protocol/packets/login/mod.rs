use std::io::{Error, ErrorKind};

use uuid::Uuid;

use crate::protocol::{packets::Packet, reader::Reader};

#[derive(Debug)]
pub struct LoginStart {
    pub username: String,
}

#[derive(Debug)]
pub struct LoginSuccess {
    pub uuid: Uuid,
    pub username: String,
}

impl Packet for LoginStart {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf.to_vec());
        let _packet_id = reader.read_varint();
        let username = reader.read_string();
        Ok(Self { username })
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x00);
        writer.write_string(&self.username);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl Packet for LoginSuccess {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::new(ErrorKind::Other, "Unexpected call"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x02);
        writer.write_string(&self.uuid.hyphenated().to_string());
        writer.write_string(&self.username);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
