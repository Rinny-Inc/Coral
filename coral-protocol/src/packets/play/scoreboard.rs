use crate::packets::PacketOut;

#[derive(Debug)]
pub struct ScoreboardObjective {
    pub name: String,
    pub mode: u8, // 0=create; 1=remove; 2=update
    pub display_name: String,
    pub render_type: String, // integer | hearts
}
impl PacketOut for ScoreboardObjective {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x3B);
        writer.write_string(&self.name);
        writer.write_byte(self.mode);
        if self.mode == 0 || self.mode == 2 {
            writer.write_string(&self.display_name);
            writer.write_string(&self.render_type);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct UpdateScore {
    pub name: String, // score holder (player name | arbitrary string)
    pub action: u8,   // 0=create/update | 1=remove
    pub objective: String,
    pub value: i32,
}
impl PacketOut for UpdateScore {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x3C);
        writer.write_string(&self.name);
        writer.write_byte(self.action);
        writer.write_string(&self.objective);
        if self.action != 1 {
            writer.write_varint(self.value);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct DisplayScoreboard {
    pub position: u8, // 0=list(tab) | 1=sidebar | 2=below name
    pub objective: String,
}
impl PacketOut for DisplayScoreboard {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x3D);
        writer.write_byte(self.position);
        writer.write_string(&self.objective);
        Ok(())
    }
}

#[derive(Debug)]
pub struct TeamPacket {
    pub name: String,
    pub mode: u8, // 0=create | 1=remove | 2=update | 3=add players | 4=remove players
    pub display_name: String,
    pub prefix: String,
    pub suffix: String,
    // TODO: replace by 2 bools "friendly_fire" & "see_friendly_invisible"
    pub friendly_fire: u8,           // 0=off | 1=on | 3=see friendly invisible
    pub name_tag_visibility: String, // "always"/"hideForOtherTeams"/"hideForOwnTeam"/"never"
    pub color: u8,
    pub players: Vec<String>,
}
impl PacketOut for TeamPacket {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x3E);
        writer.write_string(&self.name);
        writer.write_byte(self.mode);
        if self.mode == 0 || self.mode == 2 {
            writer.write_string(&self.display_name);
            writer.write_string(&self.prefix);
            writer.write_string(&self.suffix);
            writer.write_byte(self.friendly_fire);
            writer.write_string(&self.name_tag_visibility);
            writer.write_byte(self.color);
        }
        if self.mode == 0 || self.mode == 3 || self.mode == 4 {
            writer.write_varint(self.players.len() as i32);
            for p in &self.players {
                writer.write_string(p);
            }
        }
        Ok(())
    }
}
