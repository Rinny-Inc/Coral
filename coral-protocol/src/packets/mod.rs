use bytes::Bytes;
use std::collections::HashSet;

use crate::packets::handshake::EnumProtocol;

use super::writer::Writer;

pub mod handshake;
pub mod login;
pub mod play;
pub mod status;

// TODO: PacketError????

pub trait PacketIn: std::fmt::Debug + Send + Sized {
    const ID: i32;

    fn decode(buf: &mut Bytes) -> std::io::Result<Self>;
}
pub trait PacketOut: std::fmt::Debug + Send {
    fn encode(&self, writer: &mut Writer) -> std::io::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PacketKey {
    pub state: handshake::EnumProtocol,
    pub id: i32,
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub id: i32,
    pub payload: Bytes,
}
impl Frame {
    pub fn new(id: i32, payload: Bytes) -> Self {
        Self { id, payload }
    }

    pub fn decode<P: PacketIn>(&self) -> std::io::Result<P> {
        P::decode(&mut self.payload.clone())
    }

    pub fn is<P: PacketIn>(&self) -> bool {
        self.id == P::ID
    }
}

pub struct PacketRegistry {
    known: HashSet<PacketKey>,
}

macro_rules! register_packets { // GOD DAMN THIS SHIT IS STILL ALIEN AHAHAH
    ($($state:expr => $($ty:path),+ ;)+) => {{
        let mut known: HashSet<PacketKey> = HashSet::with_capacity(28); // TODO: add 1 for every new packets
        $($(
            known.insert(PacketKey {
                state: $state,
                id: <$ty as PacketIn>::ID
            });
        )+)+
        known
    }};
}

impl PacketRegistry {
    pub fn new() -> Self {
        let known = register_packets! {
           EnumProtocol::Handshaking => handshake::PacketHandshake;

           EnumProtocol::Status => status::Request, status::Ping;

           EnumProtocol::Login => login::LoginStart, login::EncryptionResponse;

           EnumProtocol::Play =>
               play::keepalive::KeepAlive,
               play::chat::ChatMessage,
               play::entity::UseEntity,
               play::movement::PlayerOnGround,
               play::movement::PlayerPosition,
               play::movement::PlayerLook,
               play::movement::PlayerPositionAndLook,
               play::block::PlayerDig,
               play::block::PlayerBlockPlacement,
               play::block::HeldItemChange,
               play::entity::ArmAnimation,
               play::entity::EntityAction,
               play::inventory::CloseWindow,
               play::inventory::ClickWindow,
               play::inventory::ConfirmTransaction,
               play::inventory::CreativeInventoryAction,
               play::entity::UpdateSign,
               play::PlayerAbilities,
               play::chat::TabComplete,
               play::ClientSettings,
               play::game::ClientStatus,
               play::PluginMessage,
               play::ResourcePackStatus;
        };

        Self { known }
    }

    pub fn knows(&self, key: &PacketKey) -> bool {
        self.known.contains(key)
    }

    pub fn ids_for(&self, state: EnumProtocol) -> impl Iterator<Item = i32> {
        self.known
            .iter()
            .filter(move |key| key.state == state)
            .map(|key| key.id)
    }

    pub fn count_for(&self, state: EnumProtocol) -> usize {
        self.ids_for(state).count()
    }
}
impl Default for PacketRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    use crate::packets::play::block::{PlayerBlockPlacement, PlayerDig};
    use crate::packets::play::chat::ChatMessage;
    use crate::packets::play::inventory::ClickWindow;
    use crate::packets::play::keepalive::KeepAlive;
    use crate::packets::play::movement::{
        PlayerLook, PlayerOnGround, PlayerPosition, PlayerPositionAndLook,
    };

    fn varint(value: i32, out: &mut Vec<u8>) {
        let mut v = value as u32;
        loop {
            if v & !0x7F == 0 {
                out.push(v as u8);
                return;
            }
            out.push((v & 0x7F) as u8 | 0x80);
            v >>= 7;
        }
    }

    fn keep_alive_frame(id: i32) -> Frame {
        let mut body = Vec::new();
        varint(id, &mut body);
        Frame::new(KeepAlive::ID, Bytes::from(body))
    }

    #[test]
    fn a_frame_decodes_its_body() {
        let frame = keep_alive_frame(1234);
        assert!(frame.is::<KeepAlive>());
        assert_eq!(frame.decode::<KeepAlive>().unwrap().id, 1234);
    }

    #[test]
    fn is_matches_only_the_type_whose_id_it_carries() {
        let frame = keep_alive_frame(1);
        assert!(frame.is::<KeepAlive>());
        assert!(!frame.is::<PlayerDig>());
        assert!(!frame.is::<ChatMessage>());
    }

    #[test]
    fn decoding_twice_gives_the_same_packet() {
        let frame = keep_alive_frame(77);

        let first = frame.decode::<KeepAlive>().unwrap();
        let second = frame.decode::<KeepAlive>().unwrap();

        assert_eq!(first.id, 77);
        assert_eq!(second.id, first.id, "the payload cursor was consumed");
        assert_eq!(frame.payload.len(), 1, "the frame's own payload shrank");
    }

    #[test]
    fn a_truncated_body_decodes_to_zeros_instead_of_failing() {
        let frame = Frame::new(KeepAlive::ID, Bytes::new());

        let decoded = frame
            .decode::<KeepAlive>()
            .expect("no decoder validates EOF, so this still succeeds");
        assert_eq!(decoded.id, 0, "the missing varint read as zero");
    }

    #[test]
    fn no_two_play_packets_share_an_id() {
        let registry = PacketRegistry::new();
        assert_eq!(
            registry.count_for(handshake::EnumProtocol::Play),
            23,
            "a Play packet was added to or dropped from the registry, or two \
             of them now declare the same ID and the set collapsed them"
        );
    }

    #[test]
    fn the_pre_play_states_keep_their_own_ids() {
        let registry = PacketRegistry::new();
        use handshake::EnumProtocol::*;

        for (state, ids) in [
            (Handshaking, vec![0x00]),
            (Status, vec![0x00, 0x01]),
            (Login, vec![0x00, 0x01]),
        ] {
            let mut accepted: Vec<i32> = registry.ids_for(state.clone()).collect();
            accepted.sort_unstable();
            assert_eq!(accepted, ids, "{state:?}");
        }
    }

    #[test]
    fn the_ids_are_the_protocol_47_wire_values() {
        assert_eq!(KeepAlive::ID, 0x00);
        assert_eq!(ChatMessage::ID, 0x01);
        assert_eq!(play::entity::UseEntity::ID, 0x02);
        assert_eq!(PlayerOnGround::ID, 0x03);
        assert_eq!(PlayerPosition::ID, 0x04);
        assert_eq!(PlayerLook::ID, 0x05);
        assert_eq!(PlayerPositionAndLook::ID, 0x06);
        assert_eq!(PlayerDig::ID, 0x07);
        assert_eq!(PlayerBlockPlacement::ID, 0x08);
        assert_eq!(play::block::HeldItemChange::ID, 0x09);
        assert_eq!(play::entity::ArmAnimation::ID, 0x0A);
        assert_eq!(play::entity::EntityAction::ID, 0x0B);
        assert_eq!(play::inventory::CloseWindow::ID, 0x0D);
        assert_eq!(ClickWindow::ID, 0x0E);
        assert_eq!(play::inventory::ConfirmTransaction::ID, 0x0F);
        assert_eq!(play::inventory::CreativeInventoryAction::ID, 0x10);
        assert_eq!(play::entity::UpdateSign::ID, 0x12);
        assert_eq!(play::PlayerAbilities::ID, 0x13);
        assert_eq!(play::chat::TabComplete::ID, 0x14);
        assert_eq!(play::ClientSettings::ID, 0x15);
        assert_eq!(play::game::ClientStatus::ID, 0x16);
        assert_eq!(play::PluginMessage::ID, 0x17);
        assert_eq!(play::ResourcePackStatus::ID, 0x19);
    }

    #[test]
    fn the_gaps_in_the_id_sequence_are_deliberate() {
        let registry = PacketRegistry::new();
        for unhandled in [0x0C, 0x11, 0x18] {
            let key = PacketKey {
                state: handshake::EnumProtocol::Play,
                id: unhandled,
            };
            assert!(
                !registry.knows(&key),
                "0x{unhandled:02X} is handled now; take it out of the gap list"
            );
        }
    }
}
