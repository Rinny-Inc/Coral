use crate::packets::PacketOut;

// 0x01
#[derive(Debug)]
pub struct JoinGame {
    pub entity_id: i32,
    pub gamemode: u8,
    pub dimension: i8,
    pub difficulty: u8,
    pub max_player: u8,
    pub level_type: String,
    pub reduced_debug_info: bool,
}

impl PacketOut for JoinGame {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x01);
        writer.write_i32(self.entity_id);
        writer.write_byte(self.gamemode);
        writer.write_byte(self.dimension as u8);
        writer.write_byte(self.difficulty);
        writer.write_byte(self.max_player);
        writer.write_string(&self.level_type);
        writer.write_bool(self.reduced_debug_info);
        Ok(())
    }
}
