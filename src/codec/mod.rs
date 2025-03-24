use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder, Framed};
use futures::SinkExt;

use crate::protocol::{reader, writer};
pub struct MinecraftCodec;

impl Decoder for MinecraftCodec {
    type Item = Vec<u8>;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }
        
        let mut reader = reader::ByteReader::new(src.to_vec());
        let length = reader.read_varint() as usize;
        
        if src.len() < length + reader.position {
            return Ok(None);
        }
        
        src.advance(reader.position);
        let data = src.split_to(length);
        Ok(Some(data.to_vec()))
    }
}

impl Encoder<Vec<u8>> for MinecraftCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: Vec<u8>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let mut writer = writer::ByteWriter::new();
        writer.write_varint(item.len() as i32);
        dst.extend_from_slice(&writer.data);
        dst.extend_from_slice(&item);
        Ok(())
    }
}

pub async fn process(socket: TcpStream) {
    let mut framed = Framed::new(socket, MinecraftCodec);

    while let Some(Ok(packet)) = framed.next().await {
        println!("Received packet: {:?}", packet);

        if packet[0] == 0x01 {
            let response = vec![0x01, 0x00, 0x00, 0x00, 0x00];
            if let Err(e) = framed.send(response).await {
                eprintln!("Error sending response: {:?}", e);
            }
        }
    }
}