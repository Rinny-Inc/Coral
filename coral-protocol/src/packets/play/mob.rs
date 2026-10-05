use coral_types::ext::AngleExt;

use crate::packets::{PacketOut, play::entity::MetadataValue};

#[derive(Debug)]
pub struct SpawnMob {
    pub entity_id: i32,
    pub mob_type: u8,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub metadata_entries: Vec<(u8, MetadataValue)>,
}
impl PacketOut for SpawnMob {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x0F);
        writer.write_varint(self.entity_id);
        writer.write_byte(self.mob_type);
        writer.write_i32((self.x * 32.0) as i32);
        writer.write_i32((self.y * 32.0) as i32);
        writer.write_i32((self.z * 32.0) as i32);
        writer.write_byte(self.yaw.to_byte());
        writer.write_byte(self.pitch.to_byte());
        writer.write_byte(self.head_yaw.to_byte());
        writer.write_i16((self.vx * 8000.0).clamp(-32768.0, 32767.0) as i16);
        writer.write_i16((self.vy * 8000.0).clamp(-32768.0, 32767.0) as i16);
        writer.write_i16((self.vz * 8000.0).clamp(-32768.0, 32767.0) as i16);

        for (index, value) in &self.metadata_entries {
            let key = (value.type_tag() << 5) | (index & 0x1F);
            writer.write_byte(key);
            value.write(writer);
        }
        writer.write_byte(0x7F);
        Ok(())
    }
}
