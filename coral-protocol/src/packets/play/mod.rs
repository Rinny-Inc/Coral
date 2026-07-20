use std::io::Error;

use crate::{
    packets::{
        PacketIn, PacketOut,
        play::{block::BlockPosition, chat::builder::ChatBuilder},
    },
    reader::Reader,
};

pub mod block;
pub mod chat;
pub mod entity;
pub mod game;
pub mod inventory;
pub mod join_game;
pub mod keepalive;
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

impl PacketOut for SpawnPosition {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x05);
        let position = BlockPosition::new(self.x, self.y as u8, self.z);

        writer.write_block_position(position);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SpawnPosition17 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl PacketOut for SpawnPosition17 {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x05);
        writer.write_i32(self.x);
        writer.write_i32(self.y);
        writer.write_i32(self.z);
        Ok(())
    }
}

impl PacketOut for PlayerAbilities {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x39);
        writer.write_byte(self.flags);
        writer.write_f32(self.fly_speed);
        writer.write_f32(self.walk_speed);
        Ok(())
    }
}
impl PacketIn for PlayerAbilities {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let flags = reader.read_byte();
        let fly_speed = reader.read_float();
        let walk_speed = reader.read_float();
        Ok(PlayerAbilities {
            flags,
            fly_speed,
            walk_speed,
        })
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
impl PacketIn for PluginMessage {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Self::decode_raw(buf)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for PluginMessage {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x3F);
        writer.write_string(&self.channel);
        writer.write_varint(self.data.len() as i32);
        writer.data.extend_from_slice(&self.data);
        Ok(())
    }
}

#[derive(Debug)]
pub struct ClientSettings {
    pub locale: String,
    pub view_distance: u8,
    pub chat_mode: u8,
    pub chat_colors: bool,
    pub skin_parts: u8,
}
impl PacketIn for ClientSettings {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let locale = reader.read_string();
        let view_distance = reader.read_byte();
        let chat_mode = reader.read_varint() as u8;
        let chat_colors = reader.read_bool();
        let skin_parts = reader.read_byte();
        Ok(ClientSettings {
            locale,
            view_distance,
            chat_mode,
            chat_colors,
            skin_parts,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct NamedSoundEffect {
    pub sound: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub volume: f32,
    pub pitch: u8,
}
impl PacketOut for NamedSoundEffect {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x29);
        writer.write_string(&self.sound);
        writer.write_i32((self.x * 8.0) as i32);
        writer.write_i32((self.y * 8.0) as i32);
        writer.write_i32((self.z * 8.0) as i32);
        writer.write_f32(self.volume);
        writer.write_byte(self.pitch);
        Ok(())
    }
}

#[derive(Debug)]
pub struct WorldParticles {
    pub particle_id: i32,
    pub long_distance: bool,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub offset_z: f32,
    pub particle_data: f32,
    pub count: i32,
}
impl PacketOut for WorldParticles {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x2A);
        writer.write_i32(self.particle_id);
        writer.write_bool(self.long_distance);
        writer.write_f32(self.x);
        writer.write_f32(self.y);
        writer.write_f32(self.z);
        writer.write_f32(self.offset_x);
        writer.write_f32(self.offset_y);
        writer.write_f32(self.offset_z);
        writer.write_f32(self.particle_data);
        writer.write_i32(self.count);
        Ok(())
    }
}

#[derive(Debug)]
pub struct Title {
    pub action: i32,
    pub text: Option<String>,
    pub fade_in: Option<i32>,
    pub stay: Option<i32>,
    pub fade_out: Option<i32>,
}
impl Title {
    pub fn show(text: &str) -> Self {
        Self {
            action: 0,
            text: Some(ChatBuilder::plain_json(text)),
            fade_in: None,
            stay: None,
            fade_out: None,
        }
    }
    pub fn subtitle(text: &str) -> Self {
        Self {
            action: 1,
            text: Some(ChatBuilder::plain_json(text)),
            fade_in: None,
            stay: None,
            fade_out: None,
        }
    }
    pub fn times(fade_in: i32, stay: i32, fade_out: i32) -> Self {
        Self {
            action: 2,
            text: None,
            fade_in: Some(fade_in),
            stay: Some(stay),
            fade_out: Some(fade_out),
        }
    }
    pub fn clear() -> Self {
        Self {
            action: 3,
            text: None,
            fade_in: None,
            stay: None,
            fade_out: None,
        }
    }
    pub fn reset() -> Self {
        Self {
            action: 4,
            text: None,
            fade_in: None,
            stay: None,
            fade_out: None,
        }
    }
}
impl PacketOut for Title {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x45);
        writer.write_varint(self.action);
        match self.action {
            0 | 1 => writer.write_string(self.text.as_deref().unwrap_or("{\"text\":\"\"}")),
            2 => {
                writer.write_i32(self.fade_in.unwrap_or(10));
                writer.write_i32(self.stay.unwrap_or(70));
                writer.write_i32(self.fade_out.unwrap_or(20));
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ResourcePackSend {
    pub url: String,
    pub hash: String,
}
impl PacketOut for ResourcePackSend {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x48);
        writer.write_string(&self.url);
        writer.write_string(&self.hash);
        Ok(())
    }
}

#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum ResourcePackResult {
    Loaded,
    Decline,
    Failed,
    Accepted,
}
impl TryFrom<u8> for ResourcePackResult {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Loaded),
            1 => Ok(Self::Decline),
            2 => Ok(Self::Failed),
            3 => Ok(Self::Accepted),
            _ => Err(value),
        }
    }
}

#[derive(Debug)]
pub struct ResourcePackStatus {
    pub hash: String,
    pub result: ResourcePackResult,
}
impl PacketIn for ResourcePackStatus {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let hash = reader.read_string();
        let result = ResourcePackResult::try_from(reader.read_varint() as u8).map_err(|e| {
            Error::new(
                std::io::ErrorKind::InvalidData,
                format!("RessourcePackStatus packet hacked: {}", e),
            )
        })?;
        Ok(ResourcePackStatus { hash, result })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
