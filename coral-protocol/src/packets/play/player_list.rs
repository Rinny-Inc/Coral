use uuid::Uuid;

use crate::{auth::ProfileProperty, packets::PacketOut};

#[derive(Debug)]
pub struct PlayerListItemAdd {
    pub uuid: Uuid,
    pub username: String,
    pub properties: Vec<ProfileProperty>,
    pub gamemode: i32,
    pub ping: i32,
}

impl PacketOut for PlayerListItemAdd {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x38);
        writer.write_varint(0);
        writer.write_varint(1);

        writer.write_uuid(&self.uuid);
        writer.write_string(&self.username);

        writer.write_varint(self.properties.len() as i32);
        for prop in &self.properties {
            writer.write_string(&prop.name);
            writer.write_string(&prop.value);
            match &prop.signature {
                Some(sig) => {
                    writer.write_bool(true);
                    writer.write_string(sig);
                }
                None => writer.write_bool(false),
            }
        }
        writer.write_varint(self.gamemode);
        writer.write_varint(self.ping);
        writer.write_bool(false);
        Ok(())
    }
}

#[derive(Debug)]
pub struct PlayerListItemRemove {
    pub uuid: Uuid,
}
impl PacketOut for PlayerListItemRemove {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x38);
        writer.write_varint(4);
        writer.write_varint(1);
        writer.write_uuid(&self.uuid);
        Ok(())
    }
}

#[derive(Debug)]
pub struct UpdateLatency {
    pub uuid: Uuid,
    pub ping: i32,
}
impl PacketOut for UpdateLatency {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x38);
        writer.write_varint(2); // update tablist
        writer.write_varint(1);
        writer.write_uuid(&self.uuid);
        writer.write_varint(self.ping);
        Ok(())
    }
}
#[derive(Debug)]
pub struct BulkUpdateLatency {
    pub entries: Vec<(Uuid, i32)>,
}
impl PacketOut for BulkUpdateLatency {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x38);
        writer.write_varint(2); // update tablist
        writer.write_varint(self.entries.len() as i32);
        for (uuid, ping) in &self.entries {
            writer.write_uuid(uuid);
            writer.write_varint(*ping);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct PlayerListItem17 {
    pub username: String,
    pub online: bool,
    pub ping: i16,
}
impl PacketOut for PlayerListItem17 {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0xC9);
        writer.write_string(&self.username);
        writer.write_bool(self.online);
        writer.write_u16(self.ping as u16);
        Ok(())
    }
}

#[derive(Debug)]
pub struct PlayerListHeaderFooter {
    pub header: String,
    pub footer: String,
}
impl PacketOut for PlayerListHeaderFooter {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x47);
        writer.write_string(&self.header);
        writer.write_string(&self.footer);
        Ok(())
    }
}

#[cfg(test)]
mod packet_id_tests {
    use super::*;
    use crate::reader::Reader;
    use crate::writer::Writer;

    #[test]
    fn header_footer_uses_its_own_packet_id() {
        let mut w = Writer::new();
        PlayerListHeaderFooter {
            header: "{\"text\":\"top\"}".into(),
            footer: "{\"text\":\"bottom\"}".into(),
        }
        .encode(&mut w)
        .unwrap();

        let mut r = Reader::new(&w.data);
        assert_eq!(r.read_varint(), 0x47);
        assert_eq!(r.read_string(), "{\"text\":\"top\"}");
        assert_eq!(r.read_string(), "{\"text\":\"bottom\"}");
        assert!(r.is_ok());
        assert_eq!(r.remaining(), 0);
    }
}
