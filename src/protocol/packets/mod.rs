use std::collections::HashMap;
use bytes::Bytes;

use super::writer::Writer;

pub mod handshake;
pub mod login;
pub mod status;

pub trait Packet: std::fmt::Debug + Send {
    fn decode(buf: &mut Bytes) -> std::io::Result<Self>
    where
        Self: Sized;

    fn encode(&self, writer: &mut Writer) -> std::io::Result<()>;

    fn as_any(&self) -> &dyn std::any::Any;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PacketKey {
    pub state: handshake::EnumProtocol,
    pub id: i32,
}
pub struct PacketParser {
    pub encoder: Option<fn(&dyn Packet, &mut Writer) -> std::io::Result<()>>,
    pub decoder: fn(&mut Bytes) -> std::io::Result<Box<dyn Packet>>
}
pub struct PacketRegistry {
    pub handlers: HashMap<PacketKey, PacketParser>,
}

impl PacketRegistry {
    pub fn new() -> Self {
        let mut handlers: HashMap<PacketKey, PacketParser> = HashMap::new();

        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Handshaking,
                id: 0x00
            }, 
            PacketParser { 
                encoder: None, 
                decoder: |buf| handshake::PacketHandshake::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>) 
            }
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Status,
                id: 0x00
            }, 
            PacketParser { 
                encoder: None, 
                decoder: |buf| status::Request::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>) 
            }
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Status,
                id: 0x01
            }, 
            PacketParser { 
                encoder: Some(|packet, writer| {
                    let pong = match packet.as_any().downcast_ref::<status::Pong>() {
                        Some(pong) => pong,
                        None => return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                       "Failed to downcast to Pong",
                        )),
                    };
                    pong.encode(writer)
                }), 
                decoder: |buf| status::Ping::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>) 
            }
        );

        Self { handlers }
    }

    pub fn parse(&self, key: PacketKey, buf: &mut Bytes) -> Option<std::io::Result<Box<dyn Packet>>> {
        self.handlers.get(&key).map(|parser| (parser.decoder)(buf))
    }

    /*fn encode(&self, key: PacketKey, packet: &dyn Packet, writer: &mut Writer) -> Option<std::io::Result<()>> {
        self.handlers.get(&key).and_then(|handler| handler.encoder.map(|encoder| encoder(packet, writer)))
    }*/
}
