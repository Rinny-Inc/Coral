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
        writer.write_i32(self.entity_id);
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
        writer.write_i32(self.entity_id);
        writer.write_byte(self.effect_id);
        Ok(())
    }
}
