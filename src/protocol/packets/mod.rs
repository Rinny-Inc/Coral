use std::collections::HashMap;

use bytes::{Bytes, BytesMut};

use handshake::{keepalive::KeepAlive, PacketHandshake};

pub mod handshake;
pub mod login;
pub mod status;

pub enum PacketsEnum {
    KeepAlive(KeepAlive),
    Handshake(PacketHandshake),
    StatusStart(status::Start),
    StatusPing(status::Ping),
    StatusDone(status::Done),
    StatusPong(status::Pong),
    LoginSuccess(login::PacketLoginSuccess),
    Unknown(Vec<u8>)
}

pub trait Packet: std::fmt::Debug {
    fn id(&self) -> i32;

    fn decode(buf: &mut Bytes) -> std::io::Result<Self>
    where
        Self: Sized;

    fn encode(&self, buf: &mut BytesMut) -> std::io::Result<()>;
}

type PacketParser = fn(&mut Bytes) -> std::io::Result<Box<dyn Packet>>;
pub struct PacketRegistry {
    handlers: HashMap<i32, PacketParser>,
}

impl PacketRegistry {
    pub fn new() -> Self {
        let mut handlers: HashMap<i32, PacketParser> = HashMap::new();

        // Register packets here
        // ex
        handlers.insert(PacketHandshake::id(), |buf| {
            PacketHandshake::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>)
        });

        Self { handlers }
    }

    pub fn parse(&self, id: i32, buf: &mut Bytes) -> Option<std::io::Result<Box<dyn Packet>>> {
        self.handlers.get(&id).map(|parser| parser(buf))
    }
}