use std::io::{Error, ErrorKind, Result};

use serde_json::json;

use crate::{
    packets::{PacketIn, PacketOut},
    reader::Reader,
    writer::Writer,
};

#[derive(Debug)]
pub struct Request;

#[derive(Debug)]
pub struct Response {
    json: String,
}

#[derive(Debug)]
pub struct Ping {
    pub time: i64,
}

#[derive(Debug)]
pub struct Pong {
    pub time: i64,
}

impl Response {
    pub fn new(
        motd: &str,
        online: u32,
        max: u32,
        protocol: i32,
        favicon: Option<&str>,
        sample: &[(&str, &str)],
    ) -> Self {
        let sample_json: Vec<serde_json::Value> = sample
            .iter()
            .map(|(name, uuid)| {
                json!({
                    "name": name,
                    "id": uuid
                })
            })
            .collect();
        let mut json = json!({
            "version": {
                "name": "Coral 1.8.9",
                "protocol": protocol
            },
            "players": {
                "max": max,
                "online": online,
                "sample": sample_json
            },
            "description": {
                "text": motd
            }
        });

        if let Some(icon) = favicon {
            json["favicon"] = json!(icon);
        }

        Self {
            json: json.to_string(),
        }
    }
}
impl PacketIn for Response {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let json = reader.read_string();
        Ok(Response { json })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for Response {
    fn encode(&self, writer: &mut Writer) -> std::io::Result<()> {
        writer.write_varint(0x00);
        writer.write_string(&self.json);
        Ok(())
    }
}

impl PacketIn for Ping {
    fn decode(buf: &mut bytes::Bytes) -> Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let time = reader.read_long(); // i64
        Ok(Ping { time })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for Ping {
    fn encode(&self, writer: &mut Writer) -> Result<()> {
        writer.write_long(self.time);
        Ok(())
    }
}

impl PacketOut for Pong {
    fn encode(&self, writer: &mut Writer) -> Result<()> {
        writer.write_varint(0x01);
        writer.write_long(self.time);
        Ok(())
    }
}

impl PacketIn for Request {
    fn decode(_buf: &mut bytes::Bytes) -> Result<Self> {
        Ok(Request)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for Request {
    fn encode(&self, writer: &mut Writer) -> Result<()> {
        writer.write_varint(0x00);
        Ok(())
    }
}
