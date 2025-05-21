use super::Packet;

pub mod keepalive;

#[derive(Debug)]
pub struct PacketHandshake {
    protocol_version: u8,
    host_name: String,
    port: u16,
    requested_protocol: EnumProtocol
}

impl Packet for PacketHandshake {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized {
        Ok(PacketHandshake { 
            protocol_version: 5, 
            host_name: "locahost".to_string(), 
            port: 25565,
            requested_protocol: EnumProtocol::Handshaking
        })
    }

    fn encode(&self, _buf: &mut bytes::BytesMut) -> std::io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EnumProtocol {
    Handshaking,
    Status,
    Login,
    Play
}