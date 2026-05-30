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
    pub fn new(motd: &str, online: u32, max: u32, protocol: i32) -> Self {
        let json = json!({
            "version": {
                "name": "Coral 1.7.x/1.8.x",
                "protocol": protocol
            },
            "players": {
                "max": max,
                "online": online,
                "sample": []
            },
            "description": {
                "text": motd
            }
        })
        .to_string();

        println!("DEBUG response json: {}", json);

        Self { json }
    }
}
impl Packet for Response {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf.to_vec());
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
        let mut reader = Reader::new(buf.to_vec());
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
        let reader = Reader::new(buf.to_vec());
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
