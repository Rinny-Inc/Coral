use std::{fmt::format, io::Error};

use crate::protocol::{packets::Packet, reader::Reader};

#[derive(Debug)]
pub struct ChatMessage {
    pub message: String,
}

#[derive(Debug)]
pub struct ChatMessageOut {
    pub json: String,
}

impl ChatMessageOut {
    pub fn new(message: &str) -> Self {
        Self {
            json: format!("{{\"text\":\"{}\"}}", message.replace('"', "\\\"")),
        }
    }

    pub fn from_json(json: &str) -> Self {
        Self {
            json: json.to_string(),
        }
    }
}

impl Packet for ChatMessage {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(&buf);
        let message = reader.read_string();
        Ok(ChatMessage { message })
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x01);
        writer.write_string(&self.message);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for ChatMessageOut {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x02);
        writer.write_string(&self.json);
        writer.write_byte(0);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
