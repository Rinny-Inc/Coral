use std::io::{Error, ErrorKind, Result};

use crate::protocol::{packets::Packet, reader::Reader, writer::Writer};

#[derive(Debug)]
pub struct Request;

pub struct Response {
    json: String
}

#[derive(Debug)]
pub struct Ping {
    pub time: i64,
}

#[derive(Debug)]
pub struct Pong {
    pub time: i64
}

impl Packet for Ping {
    fn decode(buf: &mut bytes::Bytes) -> Result<Self>
        where
            Self: Sized {
        let mut reader = Reader::new(buf.to_vec());

        let packet_id = reader.read_varint();
        if packet_id != 0x01 {
            return Err(Error::new(ErrorKind::InvalidData, format!("Expected packet ID 1, got {}", packet_id)));
        }

        let time = reader.read_long(); // i64

        if reader.has_remaining() {
            return Err(Error::new(ErrorKind::InvalidData, "Unexpected extra bytes in Ping packet"));
        }

        Ok(Ping { time })
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

impl Packet for Pong {
    fn decode(_buf: &mut bytes::Bytes) -> Result<Self> {
        return Err(Error::new(ErrorKind::Other, "Unexpected call"));
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
        let mut reader = Reader::new(buf.to_vec());
        let packet_id = reader.read_varint();
        if packet_id != 0x00 {
            return Err(Error::new(ErrorKind::InvalidData, "Expected packet ID 0 for StatusRequest"));
        }

        if reader.has_remaining() {
            return Err(Error::new(ErrorKind::InvalidData, "Unexpected extra bytes in StatusRequest"));
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