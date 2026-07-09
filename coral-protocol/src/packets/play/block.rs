use std::io::Error;

use crate::{
    packets::{PacketIn, PacketOut},
    reader::Reader,
};

#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum DigStatus {
    StartDig,
    CancelDig,
    FinishDig,
    DropItem(bool),
    ShootOrFinishEating,
}
impl TryFrom<u8> for DigStatus {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::StartDig),
            1 => Ok(Self::CancelDig),
            2 => Ok(Self::FinishDig),
            3 | 4 => Ok(Self::DropItem(value == 3)),
            5 => Ok(Self::ShootOrFinishEating),
            _ => Err(value),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
#[repr(u8)]
pub enum BlockFace {
    Down,
    Up,
    North,
    South,
    West,
    East,
}
impl TryFrom<u8> for BlockFace {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Down),
            1 => Ok(Self::Up),
            2 => Ok(Self::North),
            3 => Ok(Self::South),
            4 => Ok(Self::West),
            5 => Ok(Self::East),
            _ => Err(value),
        }
    }
}

#[derive(Debug)]
pub struct PlayerDig {
    pub status: DigStatus,
    pub x: i32,
    pub y: u8,
    pub z: i32,
    pub face: Option<BlockFace>,
}

#[derive(Debug)]
pub struct PlayerBlockPlacement {
    pub x: i32,
    pub y: u8,
    pub z: i32,
    pub face: Option<BlockFace>,
    pub held_item_id: i16,
    pub held_item_count: u8,
    pub held_item_damage: i16,
    pub cursor_x: u8,
    pub cursor_y: u8,
    pub cursor_z: u8,
}

#[derive(Debug)]
pub struct HeldItemChange {
    pub slot: i16,
}

#[derive(Debug)]
pub struct BlockChange {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block_id: i32,
    pub block_metadata: u8,
}

impl PacketIn for PlayerDig {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let status = reader.read_byte();
        let position = reader.read_long();
        let x = (position >> 38) as i32;
        let y = ((position >> 26) & 0xFFF) as u8;
        let z = (position << 38 >> 38) as i32;
        let face = match BlockFace::try_from(reader.read_byte()) {
            Ok(v) => Some(v),
            Err(_) => None,
        };
        Ok(PlayerDig {
            status: DigStatus::try_from(status).map_err(|e| {
                Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("PlayerDig hacked packet: {}", e),
                )
            })?,
            x,
            y,
            z,
            face,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketIn for PlayerBlockPlacement {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let position = reader.read_long();
        let x = (position >> 38) as i32;
        let y = ((position >> 26) & 0xFFF) as u8;
        let z = (position << 38 >> 38) as i32;
        let face = match BlockFace::try_from(reader.read_byte()) {
            Ok(v) => Some(v),
            Err(_) => None,
        };
        let held_item_id = reader.read_i16();
        let (held_item_count, held_item_damage, cursor_x, cursor_y, cursor_z) = {
            if held_item_id != -1 {
                (
                    reader.read_byte(),
                    reader.read_i16(),
                    reader.read_byte(),
                    reader.read_byte(),
                    reader.read_byte(),
                )
            } else {
                (0, 0, 0, 0, 0)
            }
        };
        Ok(PlayerBlockPlacement {
            x,
            y,
            z,
            face,
            held_item_id,
            held_item_count,
            held_item_damage,
            cursor_x,
            cursor_y,
            cursor_z,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketIn for HeldItemChange {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let slot = reader.read_i16();
        Ok(HeldItemChange { slot })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for HeldItemChange {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x09);
        writer.write_byte(self.slot as u8);
        Ok(())
    }
}
impl PacketOut for BlockChange {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x23);

        // pack position into i64
        let position: i64 = ((self.x as i64 & 0x3FFFFFF) << 38)
            | ((self.y as i64 & 0xFFF) << 26)
            | (self.z as i64 & 0x3FFFFFF);
        writer.write_long(position);

        writer.write_varint(self.block_id << 4 | self.block_metadata as i32);
        Ok(())
    }
}

#[derive(Debug)]
pub struct BlockBreakAnimation {
    pub entity_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub destroy_stage: u8,
}
impl PacketOut for BlockBreakAnimation {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x25);
        writer.write_varint(self.entity_id);
        let position: i64 = ((self.x as i64 & 0x3FFFFFF) << 38)
            | ((self.y as i64 & 0xFFF) << 26)
            | (self.z as i64 & 0x3FFFFFF);
        writer.write_long(position);
        writer.write_byte(self.destroy_stage);
        Ok(())
    }
}

#[derive(Debug)]
pub struct ItemEntityMetadata {
    pub entity_id: i32,
    pub item_id: i16,
    pub item_count: u8,
    pub item_damage: i16,
}
impl PacketOut for ItemEntityMetadata {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x1C);
        writer.write_varint(self.entity_id);
        writer.write_byte(0xAA);
        writer.write_i16(self.item_id);
        writer.write_byte(self.item_count);
        writer.write_i16(self.item_damage);
        writer.write_byte(0);
        writer.write_byte(0x7F);
        Ok(())
    }
}
