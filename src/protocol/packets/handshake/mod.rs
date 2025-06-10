use std::io::{Error, ErrorKind};

use crate::protocol::{reader::Reader, writer::Writer};

use super::Packet;

pub mod keepalive;

#[derive(Debug)]
pub struct PacketHandshake {
    protocol_version: u8,
    host_name: String,
    port: u16,
    pub requested_protocol: EnumProtocol
}

impl Packet for PacketHandshake {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized {
        let mut buffer = Reader::new(buf.to_vec());

        let _packet_id = buffer.read_varint();
        let protocol_version = buffer.read_varint() as u8;
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
            return Err(Error::new(
                ErrorKind::Other,
                format!("Bytes remaining on stream: {}", buffer.remaining()),
            ));
        }

        Ok(PacketHandshake {
            protocol_version,
            host_name,
            port,
            requested_protocol,
        })
    }

    fn encode(&self, buffer: &mut Writer) -> std::io::Result<()> {
        buffer.write_varint_byte(self.protocol_version as i8);
        buffer.write_string(&self.host_name);
        buffer.write_u16(self.port);
        buffer.write_varint_byte(self.requested_protocol.to_id());
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[repr(i8)]
pub enum EnumProtocol {
    Handshaking,
    Status,
    Login,
    Play
}

impl EnumProtocol {
    pub fn to_id(&self) -> i8 {
        self.clone() as i8
    }

    pub fn from_id(id: i8) -> Option<Self> {
        match id {
            0 => Some(Self::Handshaking),
            1 => Some(Self::Status),
            2 => Some(Self::Login),
            3 => Some(Self::Play),
            _ => None
        }
    }
}