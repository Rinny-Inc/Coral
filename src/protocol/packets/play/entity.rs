use std::io::Error;

use crate::protocol::packets::Packet;

#[derive(Debug)]
pub struct SpawnPlayer {
    pub entity_id: i32,
    pub uuid: uuid::Uuid,
    pub username: String,
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
    pub yaw: u8,
    pub pitch: u8,
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
    pub yaw: u8,
    pub pitch: u8,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct EntityLook {
    pub entity_id: i32,
    pub yaw: u8,
    pub pitch: u8,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct DestroyEntities {
    pub entity_ids: Vec<i32>,
}

#[derive(Debug)]
pub struct EntityHeadLook {
    pub entity_id: i32,
    pub head_yaw: u8,
}

fn degrees_to_byte(degrees: f32) -> u8 {
    ((degrees / 360.0 * 256.0) as i32).rem_euclid(256) as u8
}

impl Packet for SpawnPlayer {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x0C);
        writer.write_varint(self.entity_id);

        writer.write_string(&self.uuid.hyphenated().to_string());
        writer.write_string(&self.username);

        writer.write_varint(0); // properties count

        writer.write_i32((self.x * 32.0) as i32);
        writer.write_i32((self.y * 32.0) as i32);
        writer.write_i32((self.z * 32.0) as i32);
        writer.write_byte(degrees_to_byte(self.yaw));
        writer.write_byte(degrees_to_byte(self.pitch));
        writer.write_i16(self.current_item);
        writer.write_byte(127);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for EntityTeleport {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x18);
        writer.write_varint(self.entity_id);
        writer.write_i32((self.x * 32.0) as i32);
        writer.write_i32((self.y * 32.0) as i32);
        writer.write_i32((self.z * 32.0) as i32);
        writer.write_byte(self.yaw);
        writer.write_byte(self.pitch);
        writer.write_bool(self.on_ground);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for EntityRelativeMove {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x15);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.dx as u8);
        writer.write_byte(self.dy as u8);
        writer.write_byte(self.dz as u8);
        writer.write_bool(self.on_ground);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for EntityLookAndMove {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x17);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.dx as u8);
        writer.write_byte(self.dy as u8);
        writer.write_byte(self.dz as u8);
        writer.write_byte(self.yaw);
        writer.write_byte(self.pitch);
        writer.write_bool(self.on_ground);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for EntityLook {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x16);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.yaw);
        writer.write_byte(self.pitch);
        writer.write_bool(self.on_ground);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for DestroyEntities {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x13);
        writer.write_varint(self.entity_ids.len() as i32);
        for id in &self.entity_ids {
            writer.write_varint(*id);
        }
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for EntityHeadLook {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x19);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.head_yaw);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
