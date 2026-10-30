use std::io::Error;

use crate::{
    packets::{PacketIn, PacketOut},
    reader::Reader,
};

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum GameStateChangeReason {
    InvalidBed,
    EndRaining,
    BeginRaining,
    ChangeGameMode,
    EnterCredits,
    DemoMessage,
    ArrowHitPlayer,
    FadeValue,
    FadeTime,
    PlayerMobAppearance = 10,
}

#[derive(Debug)]
pub struct ChangeGameState {
    pub reason: GameStateChangeReason,
    pub value: f32,
}
impl ChangeGameState {
    pub fn set_gamemode(gamemode: u8) -> Self {
        Self {
            reason: GameStateChangeReason::ChangeGameMode,
            value: gamemode as f32,
        }
    }
}
impl PacketOut for ChangeGameState {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x2B);
        writer.write_byte(self.reason.clone() as u8);
        writer.write_f32(self.value);
        Ok(())
    }
}

#[derive(Debug)]
pub struct UpdateHealth {
    pub health: f32,
    pub food: i32,
    pub food_saturation: f32,
}

#[derive(Debug)]
pub struct ServerDifficulty {
    pub difficulty: u8,
}

#[derive(Debug)]
pub struct Respawn {
    pub dimension: i32,
    pub difficulty: u8,
    pub gamemode: u8,
    pub level_type: String,
}

#[derive(Debug)]
#[repr(u8)]
pub enum ClientStatusAction {
    PerformRespawn,
    RequestStats,
    OpenCreativeInventory,
}
impl TryFrom<u8> for ClientStatusAction {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::PerformRespawn),
            1 => Ok(Self::RequestStats),
            2 => Ok(Self::OpenCreativeInventory),
            _ => Err(value),
        }
    }
}

#[derive(Debug)]
pub struct ClientStatus {
    pub action: ClientStatusAction,
}

impl PacketOut for UpdateHealth {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x06);
        writer.write_f32(self.health);
        writer.write_varint(self.food);
        writer.write_f32(self.food_saturation);
        Ok(())
    }
}
impl PacketOut for ServerDifficulty {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x41);
        writer.write_byte(self.difficulty);
        Ok(())
    }
}
impl PacketOut for Respawn {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x07);
        writer.write_i32(self.dimension);
        writer.write_byte(self.difficulty);
        writer.write_byte(self.gamemode);
        writer.write_string(&self.level_type);
        Ok(())
    }
}
impl PacketIn for ClientStatus {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let action = ClientStatusAction::try_from(reader.read_byte()).map_err(|e| {
            Error::new(
                std::io::ErrorKind::InvalidData,
                format!("ClientStatus packet hacked: {}", e),
            )
        })?;
        Ok(ClientStatus { action })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct SetExperience {
    pub experience_bar: f32, // 0.0 = empty | 1.0 = full
    pub level: i32,
    pub total_experience: i32,
}

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum EntityStatusType {
    HurtAnimation = 2,
    DeadAnimation = 3,
    TameFailed = 6,
    TameSuccess = 7,
}

#[derive(Debug)]
pub struct EntityStatus {
    pub entity_id: i32,
    pub status: EntityStatusType,
}

impl PacketOut for SetExperience {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x1F);
        writer.write_f32(self.experience_bar);
        writer.write_varint(self.level);
        writer.write_varint(self.total_experience);
        Ok(())
    }
}
impl PacketOut for EntityStatus {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x1A);
        writer.write_i32(self.entity_id);
        writer.write_byte(self.status.clone() as u8);
        Ok(())
    }
}

#[derive(Debug)]
pub struct EntityEffect {
    pub entity_id: i32,
    pub effect_id: u8,
    pub amplifier: u8,
    pub duration: i32, // ticks
    pub hide_particles: bool,
}
impl PacketOut for EntityEffect {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x1D);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.effect_id);
        writer.write_byte(self.amplifier);
        writer.write_varint(self.duration);
        writer.write_bool(self.hide_particles);
        Ok(())
    }
}

#[derive(Debug)]
pub struct RemoveEntityEffect {
    pub entity_id: i32,
    pub effect_id: u8,
}
impl PacketOut for RemoveEntityEffect {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x1E);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.effect_id);
        Ok(())
    }
}

#[cfg(test)]
mod effect_packet_tests {
    use super::*;
    use crate::packets::PacketOut;
    use crate::reader::Reader;
    use crate::writer::Writer;

    #[test]
    fn entity_effect_fields_line_up() {
        let mut w = Writer::new();
        EntityEffect {
            entity_id: 42,
            effect_id: 19,
            amplifier: 1,
            duration: 600,
            hide_particles: false,
        }
        .encode(&mut w)
        .unwrap();

        let mut r = Reader::new(&w.data);
        assert_eq!(r.read_varint(), 0x1D);
        assert_eq!(r.read_varint(), 42);
        assert_eq!(r.read_byte(), 19);
        assert_eq!(r.read_byte(), 1);
        assert_eq!(r.read_varint(), 600);
        assert!(!r.read_bool());
        assert!(r.is_ok());
        assert_eq!(r.remaining(), 0, "trailing bytes mean a width is wrong");
    }

    #[test]
    fn remove_entity_effect_fields_line_up() {
        let mut w = Writer::new();
        RemoveEntityEffect {
            entity_id: 42,
            effect_id: 19,
        }
        .encode(&mut w)
        .unwrap();

        let mut r = Reader::new(&w.data);
        assert_eq!(r.read_varint(), 0x1E);
        assert_eq!(r.read_varint(), 42);
        assert_eq!(r.read_byte(), 19);
        assert!(r.is_ok());
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_large_entity_id_round_trips() {
        let mut w = Writer::new();
        EntityEffect {
            entity_id: 300_000,
            effect_id: 1,
            amplifier: 0,
            duration: 1,
            hide_particles: true,
        }
        .encode(&mut w)
        .unwrap();

        let mut r = Reader::new(&w.data);
        r.read_varint();
        assert_eq!(r.read_varint(), 300_000);
    }
}

#[cfg(test)]
mod server_difficulty_tests {
    use super::ServerDifficulty;
    use crate::{packets::PacketOut, writer::Writer};

    fn encoded(difficulty: u8) -> Vec<u8> {
        let mut w = Writer::new();
        ServerDifficulty { difficulty }.encode(&mut w).unwrap();
        w.data
    }

    #[test]
    fn it_is_the_id_and_one_byte() {
        assert_eq!(encoded(0), vec![0x41, 0]);
        assert_eq!(encoded(1), vec![0x41, 1]);
        assert_eq!(encoded(2), vec![0x41, 2]);
        assert_eq!(encoded(3), vec![0x41, 3]);
    }

    #[test]
    fn the_id_is_one_byte_as_a_varint() {
        assert_eq!(encoded(2).len(), 2);
    }
}
