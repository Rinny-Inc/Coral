use std::io::{Error, ErrorKind};

use crate::protocol::{
    packets::{PacketIn, PacketOut},
    reader::Reader,
    writer::Writer,
};

pub mod keepalive;

#[derive(Debug)]
pub struct PacketHandshake {
    pub protocol_version: i32,
    host_name: String,
    port: u16,
    pub requested_protocol: EnumProtocol,
}

impl PacketIn for PacketHandshake {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut buffer = Reader::new(buf);

        let protocol_version = buffer.read_varint();
        let host_name = buffer.read_string();
        let port = buffer.read_u16();
        let requested_protocol = match buffer.read_varint() {
            0 => EnumProtocol::Handshaking,
            1 => EnumProtocol::Status,
            2 => EnumProtocol::Login,
            3 => EnumProtocol::Play,
            _ => return Err(Error::new(ErrorKind::InvalidData, "Unknown protocol")),
        };

        if buffer.has_remaining() {
            return Err(Error::other(format!(
                "Bytes remaining on stream: {}",
                buffer.remaining()
            )));
        }

        Ok(PacketHandshake {
            protocol_version,
            host_name,
            port,
            requested_protocol,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for PacketHandshake {
    fn encode(&self, buffer: &mut Writer) -> std::io::Result<()> {
        buffer.write_varint(self.protocol_version);
        buffer.write_string(&self.host_name);
        buffer.write_u16(self.port);
        buffer.write_varint_byte(self.requested_protocol.to_id());
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(i8)]
pub enum EnumProtocol {
    Handshaking,
    Status,
    Login,
    Play,
}

impl EnumProtocol {
    pub fn to_id(&self) -> i8 {
        self.clone() as i8
    }
}
