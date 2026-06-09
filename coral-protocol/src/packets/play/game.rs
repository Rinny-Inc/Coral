use crate::{
    packets::{PacketIn, PacketOut},
    reader::Reader,
};

#[derive(Debug)]
pub struct ChangeGameState {
    // 0 = invalid bed
    // 1 = end raining
    // 2 = begin raining
    // 3 = change gamemode
    // 4 = enter credits
    // 5 = demo message
    // 6 = arrow hit player
    // 7 = fade value
    // 8 = fade time
    // 10 = play mob appearance
    pub reason: u8,
    pub value: f32,
}
impl ChangeGameState {
    pub fn set_gamemode(gamemode: u8) -> Self {
        Self {
            reason: 3,
            value: gamemode as f32,
        }
    }
}
impl PacketOut for ChangeGameState {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x2B);
        writer.write_byte(self.reason);
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
pub struct ClientStatus {
    // 0 = perform respawn
    // 1 = request stats
    // 2 = open inventory (creative)
    pub action: u8,
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
        let action = reader.read_byte();
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

#[derive(Debug)]
pub struct EntityStatus {
    pub entity_id: i32,
    // 2 = hurt animation + red flash
    // 3 = dead
    // 6 = tame failed (smoke particles)
    // 7 = tame succeeded (hearts)
    pub status: u8,
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
        writer.write_byte(self.status);
        Ok(())
    }
}
