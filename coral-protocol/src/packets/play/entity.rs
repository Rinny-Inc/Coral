use std::io::Error;

use crate::{
    auth::ProfileProperty,
    packets::{
        PacketIn, PacketOut,
        play::{block::BlockPosition, inventory::ItemStack},
    },
    reader::Reader,
};

#[derive(Debug)]
pub struct SpawnPlayer {
    pub entity_id: i32,
    pub uuid: uuid::Uuid,
    // pub username: String,
    pub properties: Vec<ProfileProperty>,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub current_item: i16,
}

#[derive(Debug)]
pub struct EntityTeleport {
    pub entity_id: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct EntityRelativeMove {
    pub entity_id: i32,
    pub dx: i8,
    pub dy: i8,
    pub dz: i8,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct EntityLookAndMove {
    pub entity_id: i32,
    pub dx: i8,
    pub dy: i8,
    pub dz: i8,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct EntityLook {
    pub entity_id: i32,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct DestroyEntities {
    pub entity_ids: Vec<i32>,
}

#[derive(Debug)]
pub struct EntityHeadLook {
    pub entity_id: i32,
    pub head_yaw: f32,
}

fn degrees_to_byte(degrees: f32) -> u8 {
    ((degrees * 256.0 / 360.0) as i32).rem_euclid(256) as u8
}

impl PacketOut for SpawnPlayer {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x0C);
        writer.write_varint(self.entity_id);

        writer.write_uuid(&self.uuid); // 1.7 = String UUID

        // 1.7 only
        /*
        writer.write_string(&self.username);
        writer.write_varint(self.properties.len() as i32);
        for prop in &self.properties {
            writer.write_string(&prop.name);
            writer.write_string(&prop.value);
            match &prop.signature {
                Some(sig) => {
                    writer.write_bool(true);
                    writer.write_string(sig);
                }
                None => writer.write_bool(false),
            }
        }
        */

        writer.write_varint((self.x * 32.0).floor() as i32);
        writer.write_varint((self.y * 32.0).floor() as i32);
        writer.write_varint((self.z * 32.0).floor() as i32);
        writer.write_byte(degrees_to_byte(self.yaw));
        writer.write_byte(degrees_to_byte(self.pitch));
        writer.write_i16(self.current_item);
        // metadata
        writer.write_byte(0x66);
        writer.write_f32(20.0);
        writer.write_byte(0x7F);
        Ok(())
    }
}

impl PacketOut for EntityTeleport {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x18);
        writer.write_varint(self.entity_id);
        writer.write_i32((self.x * 32.0).floor() as i32);
        writer.write_i32((self.y * 32.0).floor() as i32);
        writer.write_i32((self.z * 32.0).floor() as i32);
        writer.write_byte(degrees_to_byte(self.yaw));
        writer.write_byte(degrees_to_byte(self.pitch));
        writer.write_bool(self.on_ground);
        Ok(())
    }
}

impl PacketOut for EntityRelativeMove {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x15);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.dx as u8);
        writer.write_byte(self.dy as u8);
        writer.write_byte(self.dz as u8);
        writer.write_bool(self.on_ground);
        Ok(())
    }
}

impl PacketOut for EntityLookAndMove {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x17);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.dx as u8);
        writer.write_byte(self.dy as u8);
        writer.write_byte(self.dz as u8);
        writer.write_byte(degrees_to_byte(self.yaw));
        writer.write_byte(degrees_to_byte(self.pitch));
        writer.write_bool(self.on_ground);
        Ok(())
    }
}

impl PacketOut for EntityLook {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x16);
        writer.write_varint(self.entity_id);
        writer.write_byte(degrees_to_byte(self.yaw));
        writer.write_byte(degrees_to_byte(self.pitch));
        writer.write_bool(self.on_ground);
        Ok(())
    }
}

impl PacketOut for DestroyEntities {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x13);
        writer.write_varint(self.entity_ids.len() as i32);
        for id in &self.entity_ids {
            writer.write_varint(*id);
        }
        Ok(())
    }
}

impl PacketOut for EntityHeadLook {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x19);
        writer.write_varint(self.entity_id);
        writer.write_byte(degrees_to_byte(self.head_yaw));
        Ok(())
    }
}

#[derive(Debug)]
pub struct ArmAnimation;

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum EntityAnimationType {
    SwingArm,
    TakeDamage,
    LeaveBed,
    Eat,
    CriticalEffect(bool),
}

#[derive(Debug)]
pub struct EntityAnimation {
    pub entity_id: i32,
    pub animation: EntityAnimationType,
}

impl PacketIn for ArmAnimation {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Ok(ArmAnimation)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for EntityAnimation {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x0B);
        writer.write_varint(self.entity_id);
        let animation: u8 = match self.animation {
            EntityAnimationType::SwingArm => 0,
            EntityAnimationType::TakeDamage => 1,
            EntityAnimationType::LeaveBed => 2,
            EntityAnimationType::Eat => 3,
            EntityAnimationType::CriticalEffect(is_magic) => {
                if is_magic {
                    5
                } else {
                    4
                }
            }
        };
        writer.write_byte(animation);
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum EntityActionType {
    StartSneaking,
    StopSneaking,
    LeaveBed,
    StartSprinting,
    StopSprinting,
    HorseJump,
    OpenRiddenHorseInventory,
}
impl TryFrom<u8> for EntityActionType {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::StartSneaking),
            1 => Ok(Self::StopSneaking),
            2 => Ok(Self::LeaveBed),
            3 => Ok(Self::StartSprinting),
            4 => Ok(Self::StopSprinting),
            5 => Ok(Self::HorseJump),
            6 => Ok(Self::OpenRiddenHorseInventory),
            _ => Err(value),
        }
    }
}

#[derive(Debug)]
pub struct EntityAction {
    pub entity_id: i32,
    pub action: EntityActionType,
    pub jump_boost: i32, // Horse Only
}

#[derive(Debug)]
#[repr(u8)]
pub enum UseEntityAction {
    Interact,
    Attack,
    InteractAt,
}
impl TryFrom<u8> for UseEntityAction {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Interact),
            1 => Ok(Self::Attack),
            2 => Ok(Self::InteractAt),
            _ => Err(value),
        }
    }
}

#[derive(Debug)]
pub struct UseEntity {
    pub target_entity_id: i32,
    pub action: UseEntityAction,
}

#[derive(Debug)]
pub struct EntityVelocity {
    pub entity_id: i32,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
}

#[derive(Debug)]
pub struct EntityMetadata {
    pub entity_id: i32,
    pub entity_flags: u8, // bit 1 = sneaking, bit 3 = sprinting
    pub skin_parts: u8,
}

impl PacketIn for EntityAction {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let entity_id = reader.read_varint();
        let action = EntityActionType::try_from(reader.read_varint() as u8).map_err(|e| {
            Error::new(
                std::io::ErrorKind::InvalidData,
                format!("EntityAction packet hacked: {}", e),
            )
        })?;
        let jump_boost = reader.read_varint();
        Ok(EntityAction {
            entity_id,
            action,
            jump_boost,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketIn for UseEntity {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let target_entity_id = reader.read_varint();
        let action = UseEntityAction::try_from(reader.read_varint() as u8).map_err(|e| {
            Error::new(
                std::io::ErrorKind::InvalidData,
                format!("UseEntity packet hacked: {}", e),
            )
        })?;
        Ok(UseEntity {
            target_entity_id,
            action,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl PacketOut for EntityVelocity {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        let factor = 8000.0f64;
        let max = 3.9f64;

        writer.write_varint(0x12);
        writer.write_varint(self.entity_id);
        writer.write_i16((self.vx.clamp(-max, max) * factor) as i16);
        writer.write_i16((self.vy.clamp(-max, max) * factor) as i16);
        writer.write_i16((self.vz.clamp(-max, max) * factor) as i16);
        Ok(())
    }
}
impl PacketOut for EntityMetadata {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x1C);
        writer.write_varint(self.entity_id);

        // index 0
        writer.write_byte(0x00);
        writer.write_byte(self.entity_flags);

        // index 10 - skin layers
        writer.write_byte(0x0A);
        writer.write_byte(self.skin_parts);

        // end
        writer.write_byte(0x7F);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SpawnObject {
    pub entity_id: i32,
    pub object_type: u8,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: u8,
    pub pitch: u8,
    pub data: i32,
    pub vx: i16,
    pub vy: i16,
    pub vz: i16,
}

impl PacketOut for SpawnObject {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x0E);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.object_type);
        writer.write_i32((self.x * 32.0) as i32);
        writer.write_i32((self.y * 32.0) as i32);
        writer.write_i32((self.z * 32.0) as i32);
        writer.write_byte(self.pitch);
        writer.write_byte(self.yaw);
        writer.write_i32(self.data);
        if self.data != 0 {
            writer.write_i16(self.vx);
            writer.write_i16(self.vy);
            writer.write_i16(self.vz);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CollectItem {
    pub collected_entity_id: i32,
    pub collector_entity_id: i32,
}
impl PacketOut for CollectItem {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x0D);
        writer.write_varint(self.collected_entity_id);
        writer.write_varint(self.collector_entity_id);
        Ok(())
    }
}

#[derive(Debug)]
pub struct EntityEquipment {
    pub entity_id: i32,
    pub slot: i16,
    pub item_id: i16,
    pub count: u8,
    pub metadata: i16,
}
impl PacketOut for EntityEquipment {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x04);
        writer.write_varint(self.entity_id);
        writer.write_i16(self.slot);
        writer.write_i16(self.item_id);
        if self.item_id != -1 {
            writer.write_byte(self.count);
            writer.write_i16(self.metadata);
            writer.write_byte(0);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct SpawnExperienceOrb {
    pub entity_id: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub count: i16,
}
impl PacketOut for SpawnExperienceOrb {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x11);
        writer.write_varint(self.entity_id);
        writer.write_i32((self.x * 32.0) as i32);
        writer.write_i32((self.y * 32.0) as i32);
        writer.write_i32((self.z * 32.0) as i32);
        writer.write_i16(self.count);
        Ok(())
    }
}

#[derive(Debug)]
pub struct UseBed {
    pub entity_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl PacketOut for UseBed {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x0A);
        writer.write_varint(self.entity_id);
        let pos = BlockPosition::new(self.x, self.y as u8, self.z);
        writer.write_block_position(pos);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum TileEntity {
    Sign { lines: [String; 4] },
    Chest { items: Vec<Option<ItemStack>> },
    // Furnace(FurnaceData)
}

#[derive(Debug)]
pub struct UpdateSign {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub lines: [String; 4],
}
impl PacketOut for UpdateSign {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x33);
        let pos = BlockPosition::new(self.x, self.y as u8, self.z);
        writer.write_block_position(pos);
        for line in &self.lines {
            writer.write_string(line);
        }
        Ok(())
    }
}
impl PacketIn for UpdateSign {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let BlockPosition { x, y, z } = reader.read_block_position();

        let lines = [
            reader.read_string(),
            reader.read_string(),
            reader.read_string(),
            reader.read_string(),
        ];
        Ok(UpdateSign {
            x,
            y: y as i32,
            z,
            lines,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
