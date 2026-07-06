use bytes::Bytes;
use std::collections::HashMap;

use crate::packets::handshake::EnumProtocol;

use super::writer::Writer;

pub mod handshake;
pub mod login;
pub mod play;
pub mod status;

// TODO: PacketError????

pub trait PacketIn: std::fmt::Debug + Send {
    fn decode(buf: &mut Bytes) -> std::io::Result<Self>
    where
        Self: Sized;

    fn as_any(&self) -> &dyn std::any::Any;
}
pub trait PacketOut: std::fmt::Debug + Send {
    fn encode(&self, writer: &mut Writer) -> std::io::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PacketKey {
    pub state: handshake::EnumProtocol,
    pub id: i32,
}

type DecoderFn = fn(&mut Bytes) -> std::io::Result<Box<dyn PacketIn>>;
pub struct PacketRegistry {
    pub handlers: HashMap<PacketKey, DecoderFn>,
}

macro_rules! register_packet_handers { // GOD DAMN THIS SHIT IS STILL ALIEN AHAHAH
    ($($state:expr, $id:expr => $ty:path;)+) => {{
        let mut handlers: HashMap<PacketKey, DecoderFn> = HashMap::with_capacity(26); // TODO: add 1 for every new packets
        $(
            handlers.insert(
                PacketKey {
                    state: $state,
                    id: $id
                },
                |buf| <$ty>::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
            );
        )+
        handlers
    }};
}

impl PacketRegistry {
    pub fn new() -> Self {
        let handlers = register_packet_handers! {
            EnumProtocol::Handshaking, 0x00 => handshake::PacketHandshake;

            EnumProtocol::Status, 0x00 => status::Request;
            EnumProtocol::Status, 0x01 => status::Ping;

            EnumProtocol::Login, 0x00 => login::LoginStart;
            EnumProtocol::Login, 0x01 => login::EncryptionResponse;

            EnumProtocol::Play, 0x00 => play::keepalive::KeepAlive;
            EnumProtocol::Play, 0x01 => play::chat::ChatMessage;
            EnumProtocol::Play, 0x03 => play::movement::PlayerOnGround;
            EnumProtocol::Play, 0x04 => play::movement::PlayerPosition;
            EnumProtocol::Play, 0x05 => play::movement::PlayerLook;
            EnumProtocol::Play, 0x06 => play::movement::PlayerPositionAndLook;
            EnumProtocol::Play, 0x07 => play::block::PlayerDig;
            EnumProtocol::Play, 0x08 => play::block::PlayerBlockPlacement;
            EnumProtocol::Play, 0x09 => play::block::HeldItemChange;
            EnumProtocol::Play, 0x10 => play::inventory::CreativeInventoryAction;
            EnumProtocol::Play, 0x13 => play::PlayerAbilities;
            EnumProtocol::Play, 0x14 => play::chat::TabComplete;
            EnumProtocol::Play, 0x15 => play::ClientSettings;
            EnumProtocol::Play, 0x16 => play::game::ClientStatus;
            EnumProtocol::Play, 0x17 => play::PluginMessage;
            EnumProtocol::Play, 0x19 => play::ResourcePackStatus;
            EnumProtocol::Play, 0x0A => play::entity::ArmAnimation;
            EnumProtocol::Play, 0x02 => play::entity::UseEntity;
            EnumProtocol::Play, 0x0B => play::entity::EntityAction;
            EnumProtocol::Play, 0x0D => play::inventory::CloseWindow;
            EnumProtocol::Play, 0x0E => play::inventory::ClickWindow;
            EnumProtocol::Play, 0x0F => play::inventory::ConfirmTransaction;
        };

        Self { handlers }
    }

    pub fn parse(
        &self,
        key: PacketKey,
        buf: &mut Bytes,
    ) -> Option<std::io::Result<Box<dyn PacketIn>>> {
        self.handlers.get(&key).map(|decoder| decoder(buf))
    }
}
