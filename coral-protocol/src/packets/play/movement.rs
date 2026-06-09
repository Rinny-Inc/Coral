use crate::{packets::PacketIn, reader::Reader};

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
pub struct PlayerPositionAndLookIn {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

#[derive(Debug)]
pub struct PlayerOnGround {
    pub on_ground: bool,
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

impl PacketIn for PlayerPositionAndLookIn {
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
        Ok(PlayerPositionAndLookIn {
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
