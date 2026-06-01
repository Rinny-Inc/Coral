use bytes::Bytes;
use std::collections::HashMap;

use super::writer::Writer;

pub mod handshake;
pub mod login;
pub mod play;
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

type DecoderFn = fn(&mut Bytes) -> std::io::Result<Box<dyn Packet>>;
pub struct PacketRegistry {
    pub handlers: HashMap<PacketKey, DecoderFn>,
}

impl PacketRegistry {
    pub fn new() -> Self {
        let mut handlers: HashMap<PacketKey, DecoderFn> = HashMap::with_capacity(12); // TODO: add 1 for every new packets

        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Handshaking,
                id: 0x00,
            },
            |buf| handshake::PacketHandshake::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>),
        );

        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Status,
                id: 0x00,
            },
            |buf| status::Request::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>),
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Status,
                id: 0x01,
            },
            |buf| status::Ping::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>),
        );

        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Login,
                id: 0x00,
            },
            |buf| login::LoginStart::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>),
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Login,
                id: 0x01,
            },
            |buf| login::EncryptionResponse::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>),
        );

        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Play,
                id: 0x00,
            },
            |buf| {
                handshake::keepalive::KeepAlive::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>)
            },
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Play,
                id: 0x01,
            },
            |buf| play::chat::ChatMessage::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>),
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Play,
                id: 0x03,
            },
            |buf| {
                play::movement::PlayerOnGround::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>)
            },
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Play,
                id: 0x04,
            },
            |buf| {
                play::movement::PlayerPosition::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>)
            },
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Play,
                id: 0x05,
            },
            |buf| play::movement::PlayerLook::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>),
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Play,
                id: 0x06,
            },
            |buf| {
                play::movement::PlayerPositionAndLookIn::decode(buf)
                    .map(|p| Box::new(p) as Box<dyn Packet>)
            },
        );
        handlers.insert(
            PacketKey {
                state: handshake::EnumProtocol::Play,
                id: 0x17,
            },
            |buf| play::PluginMessage::decode(buf).map(|p| Box::new(p) as Box<dyn Packet>),
        );

        Self { handlers }
    }

    pub fn parse(
        &self,
        key: PacketKey,
        buf: &mut Bytes,
    ) -> Option<std::io::Result<Box<dyn Packet>>> {
        self.handlers.get(&key).map(|decoder| decoder(buf))
    }
}
