use uuid::Uuid;

use crate::{
    packets::{PacketIn, PacketOut},
    reader::Reader,
};

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

impl PacketIn for LoginStart {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let username = reader.read_string();
        Ok(Self { username })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for LoginStart {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x00);
        writer.write_string(&self.username);
        Ok(())
    }
}
impl PacketOut for LoginSuccess {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x02);
        writer.write_string(&self.uuid.hyphenated().to_string());
        writer.write_string(&self.username);
        Ok(())
    }
}

impl PacketOut for EncryptionRequest {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x01);
        writer.write_string(&self.server_id);
        writer.write_varint(self.public_key.len() as i32);
        writer.data.extend_from_slice(&self.public_key);
        writer.write_varint(self.verify_token.len() as i32);
        writer.data.extend_from_slice(&self.verify_token);
        Ok(())
    }
}
impl PacketIn for EncryptionResponse {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let secret_len = reader.read_varint() as usize;
        let shared_secret = reader.read_bytes(secret_len);
        let token_len = reader.read_varint() as usize;
        let verify_token = reader.read_bytes(token_len);
        Ok(EncryptionResponse {
            shared_secret,
            verify_token,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[derive(Debug)]
pub struct SetCompression {
    pub threshold: i32,
}
impl PacketOut for SetCompression {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x03);
        writer.write_varint(self.threshold);
        Ok(())
    }
}
