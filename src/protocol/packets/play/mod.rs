use std::io::Error;

use crate::protocol::{packets::Packet, reader::Reader};

pub mod chat;
pub mod entity;
pub mod join_game;
pub mod movement;
pub mod player_list;

// 0x05
#[derive(Debug)]
pub struct SpawnPosition {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

// 0x39
#[derive(Debug)]
pub struct PlayerAbilities {
    pub flags: u8,
    pub fly_speed: f32,
    pub walk_speed: f32,
}

// 0x08
#[derive(Debug)]
pub struct PlayerPositionAndLook {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

impl Packet for SpawnPosition {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x05);
        let packed: i64 = ((self.x as i64 & 0x3FFFFFF) << 38)
            | ((self.y as i64 & 0xFFF) << 26)
            | (self.z as i64 & 0x3FFFFFF);

        writer.write_long(packed);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct SpawnPosition17 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Packet for SpawnPosition17 {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x05);
        writer.write_i32(self.x);
        writer.write_i32(self.y);
        writer.write_i32(self.z);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for PlayerAbilities {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x39);
        writer.write_byte(self.flags);
        writer.write_f32(self.fly_speed);
        writer.write_f32(self.walk_speed);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for PlayerPositionAndLook {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x08);
        writer.write_f64(self.x);
        writer.write_f64(self.y);
        writer.write_f64(self.z);
        writer.write_f32(self.yaw);
        writer.write_f32(self.pitch);
        writer.write_bool(self.on_ground);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct PluginMessage {
    pub channel: String,
    pub data: Vec<u8>,
}

impl PluginMessage {
    pub fn decode_raw(buf: &mut bytes::Bytes) -> std::io::Result<Self> {
        let mut reader = Reader::new(buf);
        let channel = reader.read_string();
        let data = reader.read_bytes(reader.remaining());
        Ok(PluginMessage { channel, data })
    }

    pub fn brand(name: &str) -> Self {
        let mut data = Vec::new();
        let bytes = name.as_bytes();

        let mut len = bytes.len() as i32;
        while (len & !0x7F) != 0 {
            data.push(((len & 0x7F) as u8) | 0x80);
            len >>= 7;
        }
        data.push(len as u8);
        data.extend_from_slice(bytes);

        PluginMessage {
            channel: "MC|Brand".to_string(),
            data,
        }
    }
}
impl Packet for PluginMessage {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Self::decode_raw(buf)
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x3F);
        writer.write_string(&self.channel);
        writer.write_varint(self.data.len() as i32);
        writer.data.extend_from_slice(&self.data);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
