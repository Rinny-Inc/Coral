use std::sync::Arc;

use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use bytes::{Buf, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder, Framed};
use futures::SinkExt;

use crate::protocol::{packets::{handshake::EnumProtocol, Packet, PacketKey, PacketRegistry}, reader::Reader, writer::Writer};
pub struct Codec {
    pub registry: Arc<PacketRegistry>,
    pub state: EnumProtocol,
}

impl Decoder for Codec {
    type Item = Box<dyn Packet>;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }
        
        let mut reader = Reader::new(src.to_vec());
        let length = reader.read_varint() as usize;
        
        if src.len() < length + reader.position {
            return Ok(None);
        }
        
        src.advance(reader.position);
        let data = src.split_to(length);

        let mut bytes = Bytes::from(data.to_vec());
        let mut inner_reader = Reader::new(bytes.clone().to_vec());

        let id = inner_reader.read_varint();
        let key = PacketKey {
            state: self.state.clone(),
            id
        };

        match self.registry.parse(key, &mut bytes) {
            Some(Ok(packet)) => Ok(Some(packet)),
            Some(Err(e)) => Err(e),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData, 
                "Unknown packet ID"
            ))
        }
    }
}

impl Encoder<Box<dyn Packet>> for Codec {
    type Error = std::io::Error;

    fn encode(&mut self, item: Box<dyn Packet>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let mut writer = Writer::new();
        item.encode(&mut writer)?;
        
        let data = writer.data;
        let mut length_writer = Writer::new();
        length_writer.write_varint(data.len() as i32);
        dst.extend_from_slice(&length_writer.data);
        dst.extend_from_slice(&data);
        Ok(())
    }
}

pub async fn process(socket: TcpStream, registry: Arc<PacketRegistry>) {
    let codec = Codec {
        registry,
        state: EnumProtocol::Handshaking
    };
    let mut framed = Framed::new(socket, codec);

    while let Some(result) = framed.next().await {
        match result {
            Ok(packet) => {
                println!("Received packet: {:?}", packet);
                break;
            },
            Err(e) => {
                eprintln!("Error processing packet: {:?}", e);
                break;
            }
        }
    }
}