use uuid::Uuid;

use crate::{
    packets::{PacketIn, PacketOut},
    reader::Reader,
};

#[derive(Debug)]
pub struct PlayerPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct PlayerLook {
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct PlayerOnGround {
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct PlayerPositionAndLook {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

impl PacketOut for PlayerPositionAndLook {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x08);
        writer.write_f64(self.x);
        writer.write_f64(self.y);
        writer.write_f64(self.z);
        writer.write_f32(self.yaw);
        writer.write_f32(self.pitch);
        writer.write_bool(self.on_ground);
        Ok(())
    }
}

impl PacketIn for PlayerPositionAndLook {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let x = reader.read_double();
        let y = reader.read_double();
        let z = reader.read_double();
        let yaw = reader.read_float();
        let pitch = reader.read_float();
        let on_ground = reader.read_bool();
        Ok(PlayerPositionAndLook {
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl PacketIn for PlayerPosition {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let x = reader.read_double();
        let y = reader.read_double();
        let z = reader.read_double();
        let on_ground = reader.read_bool();
        Ok(PlayerPosition { x, y, z, on_ground })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl PacketIn for PlayerLook {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let yaw = reader.read_float();
        let pitch = reader.read_float();
        let on_ground = reader.read_bool();
        Ok(PlayerLook {
            yaw,
            pitch,
            on_ground,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl PacketIn for PlayerOnGround {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let on_ground = reader.read_bool();
        Ok(PlayerOnGround { on_ground })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug, Clone)]
pub struct PlayerMovements {
    pub position: Option<(f64, f64, f64)>,
    pub rotation: Option<(f32, f32)>,
    pub on_ground: bool,
}
impl From<&PlayerPosition> for PlayerMovements {
    fn from(p: &PlayerPosition) -> Self {
        Self {
            position: Some((p.x, p.y, p.z)),
            rotation: None,
            on_ground: p.on_ground,
        }
    }
}
impl From<&PlayerLook> for PlayerMovements {
    fn from(p: &PlayerLook) -> Self {
        Self {
            position: None,
            rotation: Some((p.yaw, p.pitch)),
            on_ground: p.on_ground,
        }
    }
}
impl From<&PlayerPositionAndLook> for PlayerMovements {
    fn from(p: &PlayerPositionAndLook) -> Self {
        Self {
            position: Some((p.x, p.y, p.z)),
            rotation: Some((p.yaw, p.pitch)),
            on_ground: p.on_ground,
        }
    }
}
impl From<&PlayerOnGround> for PlayerMovements {
    fn from(p: &PlayerOnGround) -> Self {
        Self {
            position: None,
            rotation: None,
            on_ground: p.on_ground,
        }
    }
}

#[derive(Clone)]
pub struct MovementBroadcast {
    pub uuid: Uuid,
    pub entity_id: i32,
    pub kind: MoveKind,
    pub head_yaw: Option<f32>,
}

#[derive(Clone)]
pub enum MoveKind {
    Relative {
        dx: i8,
        dy: i8,
        dz: i8,
        on_ground: bool,
    },
    Look {
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    LookAndRelative {
        dx: i8,
        dy: i8,
        dz: i8,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    Teleport {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
}
