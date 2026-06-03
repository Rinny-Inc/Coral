use std::io::Error;

use crate::protocol::{packets::Packet, reader::Reader};

#[derive(Debug)]
pub struct PlayerDig {
    pub status: u8,
    pub x: i32,
    pub y: u8,
    pub z: i32,
    pub face: u8,
}

#[derive(Debug)]
pub struct PlayerBlockPlacement {
    pub x: i32,
    pub y: u8,
    pub z: i32,
    pub face: u8,
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

impl Packet for PlayerDig {
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
        let face = reader.read_byte();
        Ok(PlayerDig {
            status,
            x,
            y,
            z,
            face,
        })
    }

    fn encode(&self, _writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        Err(Error::other("Unexpected Call!"))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl Packet for PlayerBlockPlacement {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let position = reader.read_long();
        let x = (position >> 38) as i32;
        let y = ((position >> 26) & 0xFFF) as u8;
        let z = (position << 38 >> 38) as i32;
        let face = reader.read_byte();
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

    fn encode(&self, _writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        Err(Error::other("Unexpected Call!"))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl Packet for HeldItemChange {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let slot = reader.read_i16();
        Ok(HeldItemChange { slot })
    }

    fn encode(&self, _writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        Err(Error::other("Unexpected Call!"))
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl Packet for BlockChange {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x23);

        // pack position into i64
        let position: i64 = ((self.x as i64 & 0x3FFFFFF) << 38)
            | ((self.y as i64 & 0xFFF) << 26)
            | (self.z as i64 & 0x3FFFFFF);
        writer.write_long(position);

        writer.write_varint(self.block_id << 4 | self.block_metadata as i32);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
