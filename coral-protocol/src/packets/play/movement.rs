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
