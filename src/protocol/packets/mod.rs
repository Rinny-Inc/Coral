use std::collections::HashMap;

use bytes::{Bytes, BytesMut};
use handshake::keepalive::KeepAlive;

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

    fn encode(&self) -> std::io::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PacketKey {
    pub state: handshake::EnumProtocol,
    pub id: i32,
}
type PacketParser = fn(&mut Bytes) -> std::io::Result<Box<dyn Packet>>;
pub struct PacketRegistry {
    handlers: HashMap<PacketKey, PacketParser>,
}

impl PacketRegistry {
    pub fn new() -> Self {
        let mut handlers: HashMap<PacketKey, PacketParser> = HashMap::new();

        // Register packets here
        // ex
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Handshaking,
                id: 0x00
            }, 
            |buf| handshake::PacketHandshake::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>)
        );

        Self { handlers }
    }

    pub fn parse(&self, key: PacketKey, buf: &mut Bytes) -> Option<std::io::Result<Box<dyn Packet>>> {
        self.handlers.get(&key).map(|parser| parser(buf))
    }
}
