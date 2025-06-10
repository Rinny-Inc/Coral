use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use bytes::{Buf, Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder, Framed};

use crate::protocol::{packets::{handshake::{EnumProtocol, PacketHandshake}, Packet, PacketKey, PacketRegistry}, reader::Reader, writer::Writer};
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

        println!("Decoder data -> {:?}", data.to_vec());

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
        registry: registry.clone(),
        state: EnumProtocol::Handshaking
    };
    let mut framed = Framed::new(socket, codec);

    while let Some(result) = framed.next().await {
        match result {
            Ok(packet) => {
                println!("Received packet: {:?}", packet);

                if let Some(handshake) = packet.as_any().downcast_ref::<PacketHandshake>() {
                    println!("Handshake received. Changing state to {:?}", handshake.requested_protocol);
                    framed.codec_mut().state = handshake.requested_protocol.clone();
                    continue; 
                }

                // TEST
                let mut encoded_buf = BytesMut::new();
                let mut should_send = false;
                
                for (_, parser) in &registry.handlers {
                    if let Some(encoder_fn) = parser.encoder {
                        let mut writer = Writer::new();
                        if encoder_fn(packet.as_ref(), &mut writer).is_ok() {
                            let mut length_writer = Writer::new();
                            length_writer.write_varint(writer.data.len() as i32);

                            encoded_buf.extend_from_slice(&length_writer.data);
                            encoded_buf.extend_from_slice(&writer.data);

                            should_send = true;
                            break;
                        }
                    }
                }
                if should_send {
                    if let Err(e) = framed.get_mut().write_all(&encoded_buf).await {
                        eprintln!("Error sending encoded packet: {:?}", e);
                    }
                }
                // TEST
                continue;
            },
            Err(e) => {
                eprintln!("Error processing packet: {:?}", e);
                continue;
            }
        }
    }
}