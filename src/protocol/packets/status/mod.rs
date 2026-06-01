use std::io::{Error, ErrorKind, Result};

use serde_json::json;

use crate::protocol::{packets::Packet, reader::Reader, writer::Writer};

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
        let json = if let Some(icon) = favicon {
            json!({
                "version": {
                    "name": "Coral 1.7.x/1.8.x",
                    "protocol": protocol
                },
                "players": {
                    "max": max,
                    "online": online,
                    "sample": sample_json
                },
                "description": {
                    "text": motd
                },
                "favicon": icon
            })
        } else {
            json!({
                "version": {
                    "name": "Coral 1.7.x/1.8.x",
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
            })
        };

        Self {
            json: json.to_string(),
        }
    }
}
impl Packet for Response {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let json = reader.read_string();
        Ok(Response { json })
    }

    fn encode(&self, writer: &mut Writer) -> std::io::Result<()> {
        writer.write_varint(0x00);
        writer.write_string(&self.json);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for Ping {
    fn decode(buf: &mut bytes::Bytes) -> Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let time = reader.read_long(); // i64

        if reader.has_remaining() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Unexpected extra bytes in Ping packet",
            ));
        }

        Ok(Ping { time })
    }

    fn encode(&self, writer: &mut Writer) -> Result<()> {
        writer.write_long(self.time);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for Pong {
    fn decode(_buf: &mut bytes::Bytes) -> Result<Self> {
        Err(Error::other("Unexpected call"))
    }

    fn encode(&self, writer: &mut Writer) -> Result<()> {
        writer.write_varint(0x01);
        writer.write_long(self.time);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for Request {
    fn decode(buf: &mut bytes::Bytes) -> Result<Self> {
        let reader = Reader::new(buf);
        if reader.has_remaining() {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Unexpected extra bytes in StatusRequest",
            ));
        }

        Ok(Request)
    }

    fn encode(&self, writer: &mut Writer) -> Result<()> {
        writer.write_varint(0x00);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
