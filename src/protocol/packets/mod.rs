use std::collections::HashMap;

use bytes::{Bytes, BytesMut};
use handshake::keepalive::KeepAlive;

use super::writer::Writer;

pub mod handshake;
pub mod login;
pub mod status;

pub enum PacketsEnum {
    KeepAlive(KeepAlive),
    Handshake(handshake::PacketHandshake),
    StatusStart(status::Start),
    StatusPing(status::Ping),
    StatusDone(status::Done),
    StatusPong(status::Pong),
    LoginSuccess(login::PacketLoginSuccess),
    Unknown(Vec<u8>)
}

pub trait Packet: std::fmt::Debug {
    fn decode(buf: &mut Bytes) -> std::io::Result<Self>
    where
        Self: Sized;

    fn encode(&self, writer: &mut Writer) -> std::io::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PacketKey {
    pub state: handshake::EnumProtocol,
    pub id: i32,
}
pub struct PacketStruct {
    encoder: fn(&mut Bytes) -> std::io::Result<()>,
    decoder: fn(&mut Bytes) -> std::io::Result<Box<dyn Packet>>
}
type PacketParser = fn(&mut Bytes) -> std::io::Result<Box<dyn Packet>>;
pub struct PacketRegistry {
    handlers: HashMap<PacketKey, PacketParser>,
}

impl PacketRegistry {
    pub fn new() -> Self {
        let mut handlers: HashMap<PacketKey, PacketParser> = HashMap::new();

        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Handshaking,
                id: 0x00
            }, 
            |buf| handshake::PacketHandshake::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>)
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Status,
                id: 0x00
            }, 
            |buf| status::Start::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>)
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Status,
                id: 0x01
            }, 
            |buf| status::Ping::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>)
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Status,
                id: 0x01
            }, 
            |buf| status::Pong::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>)
        );

        Self { handlers }
    }

    pub fn parse(&self, key: PacketKey, buf: &mut Bytes) -> Option<std::io::Result<Box<dyn Packet>>> {
        self.handlers.get(&key).map(|parser| parser(buf))
    }
}
