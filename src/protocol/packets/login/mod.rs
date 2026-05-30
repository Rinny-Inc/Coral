use std::io::Error;

use uuid::Uuid;

use crate::protocol::{packets::Packet, reader::Reader};

pub mod disconnect;

#[derive(Debug)]
pub struct LoginStart {
    pub username: String,
}

#[derive(Debug)]
pub struct LoginSuccess {
    pub uuid: Uuid,
    pub username: String,
}

#[derive(Debug)]
pub struct EncryptionRequest {
    pub server_id: String,
    pub public_key: Vec<u8>,
    pub verify_token: Vec<u8>,
}

#[derive(Debug)]
pub struct EncryptionResponse {
    pub shared_secret: Vec<u8>,
    pub verify_token: Vec<u8>,
}

impl Packet for LoginStart {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf.to_vec());
        let username = reader.read_string();
        Ok(Self { username })
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x00);
        writer.write_string(&self.username);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl Packet for LoginSuccess {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected call"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x02);
        writer.write_string(&self.uuid.hyphenated().to_string());
        writer.write_string(&self.username);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Packet for EncryptionRequest {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x01);
        writer.write_string(&self.server_id);
        writer.write_varint(self.public_key.len() as i32);
        writer.data.extend_from_slice(&self.public_key);
        writer.write_varint(self.verify_token.len() as i32);
        writer.data.extend_from_slice(&self.verify_token);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl Packet for EncryptionResponse {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf.to_vec());
        let secret_len = reader.read_varint() as usize;
        let shared_secret = reader.read_bytes(secret_len);
        let token_len = reader.read_varint() as usize;
        let verify_token = reader.read_bytes(token_len);
        Ok(EncryptionResponse {
            shared_secret,
            verify_token,
        })
    }

    fn encode(&self, _writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        Err(Error::other("Unexpected Call!"))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
