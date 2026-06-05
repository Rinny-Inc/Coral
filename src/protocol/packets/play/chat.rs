use crate::protocol::{
    packets::{PacketIn, PacketOut},
    reader::Reader,
};

#[derive(Debug)]
pub struct ChatMessage {
    pub message: String,
}

#[derive(Debug)]
pub struct ChatMessageOut {
    pub json: String,
}

impl ChatMessageOut {
    pub fn from_json(json: &str) -> Self {
        Self {
            json: json.to_string(),
        }
    }
}

impl PacketIn for ChatMessage {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let message = reader.read_string();
        Ok(ChatMessage { message })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for ChatMessage {
    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x01);
        writer.write_string(&self.message);
        Ok(())
    }
}

impl PacketOut for ChatMessageOut {
    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x02);
        writer.write_string(&self.json);
        writer.write_byte(0);
        Ok(())
    }
}

#[derive(Debug)]
pub struct TabComplete {
    pub text: String,
    pub has_position: bool,
    pub position: Option<i64>,
}

#[derive(Debug)]
pub struct TabCompleteResponse {
    pub matches: Vec<String>,
}

impl PacketIn for TabComplete {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let text = reader.read_string();
        let has_position = reader.read_bool();
        let position = if has_position {
            Some(reader.read_long())
        } else {
            None
        };

        Ok(TabComplete {
            text,
            has_position,
            position,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl PacketOut for TabCompleteResponse {
    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x3A);
        writer.write_varint(self.matches.len() as i32);
        for m in &self.matches {
            writer.write_string(m);
        }
        Ok(())
    }
}
