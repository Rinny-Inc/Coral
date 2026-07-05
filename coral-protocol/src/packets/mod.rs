use bytes::Bytes;
use std::collections::HashMap;

use crate::packets::handshake::EnumProtocol;

use super::writer::Writer;

pub mod handshake;
pub mod login;
pub mod play;
pub mod status;

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

impl PacketRegistry {
    pub fn new() -> Self {
        let mut handlers: HashMap<PacketKey, DecoderFn> = HashMap::with_capacity(26); // TODO: add 1 for every new packets

        handlers.insert(
            PacketKey {
                state: EnumProtocol::Handshaking,
                id: 0x00,
            },
            |buf| handshake::PacketHandshake::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );

        handlers.insert(
            PacketKey {
                state: EnumProtocol::Status,
                id: 0x00,
            },
            |buf| status::Request::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Status,
                id: 0x01,
            },
            |buf| status::Ping::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );

        handlers.insert(
            PacketKey {
                state: EnumProtocol::Login,
                id: 0x00,
            },
            |buf| login::LoginStart::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Login,
                id: 0x01,
            },
            |buf| login::EncryptionResponse::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );

        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x00,
            },
            |buf| {
                handshake::keepalive::KeepAlive::decode(buf)
                    .map(|p| Box::new(p) as Box<dyn PacketIn>)
            },
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x01,
            },
            |buf| play::chat::ChatMessage::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x03,
            },
            |buf| {
                play::movement::PlayerOnGround::decode(buf)
                    .map(|p| Box::new(p) as Box<dyn PacketIn>)
            },
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x04,
            },
            |buf| {
                play::movement::PlayerPosition::decode(buf)
                    .map(|p| Box::new(p) as Box<dyn PacketIn>)
            },
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x05,
            },
            |buf| play::movement::PlayerLook::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x06,
            },
            |buf| {
                play::movement::PlayerPositionAndLook::decode(buf)
                    .map(|p| Box::new(p) as Box<dyn PacketIn>)
            },
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x07,
            },
            |buf| play::block::PlayerDig::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x08,
            },
            |buf| {
                play::block::PlayerBlockPlacement::decode(buf)
                    .map(|p| Box::new(p) as Box<dyn PacketIn>)
            },
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x09,
            },
            |buf| {
                play::block::HeldItemChange::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>)
            },
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x10,
            },
            |buf| {
                play::inventory::CreativeInventoryAction::decode(buf)
                    .map(|p| Box::new(p) as Box<dyn PacketIn>)
            },
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x13,
            },
            |buf| play::PlayerAbilities::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x14,
            },
            |buf| play::chat::TabComplete::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x15,
            },
            |buf| play::ClientSettings::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x16,
            },
            |buf| play::game::ClientStatus::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x17,
            },
            |buf| play::PluginMessage::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x19,
            },
            |buf| play::ResourcePackStatus::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x0A,
            },
            |buf| play::entity::ArmAnimation::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x02,
            },
            |buf| play::entity::UseEntity::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x0B,
            },
            |buf| play::entity::EntityAction::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>),
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x0D,
            },
            |buf| {
                play::inventory::CloseWindow::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>)
            },
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x0E,
            },
            |buf| {
                play::inventory::ClickWindow::decode(buf).map(|p| Box::new(p) as Box<dyn PacketIn>)
            },
        );
        handlers.insert(
            PacketKey {
                state: EnumProtocol::Play,
                id: 0x0F,
            },
            |buf| {
                play::inventory::ConfirmTransaction::decode(buf)
                    .map(|p| Box::new(p) as Box<dyn PacketIn>)
            },
        );

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
