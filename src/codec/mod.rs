use std::sync::Arc;

use bytes::{Buf, Bytes, BytesMut};
use futures::SinkExt;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Encoder, Framed};

use crate::protocol::{
    packets::{
        Packet, PacketKey, PacketRegistry,
        handshake::{EnumProtocol, PacketHandshake},
        status::{Ping, Pong, Request, Response},
    },
    reader::Reader,
    writer::Writer,
};
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

        println!("INFO: Decoder Codec data -> {:?}", data.to_vec());

        let mut bytes = Bytes::from(data.to_vec());
        let mut inner_reader = Reader::new(bytes.clone().to_vec());

        let id = inner_reader.read_varint();
        let key = PacketKey {
            state: self.state.clone(),
            id,
        };

        match self.registry.parse(key, &mut bytes) {
            Some(Ok(packet)) => Ok(Some(packet)),
            Some(Err(e)) => Err(e),
            None => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Unknown packet ID: {}", id),
            )),
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

async fn send_packet<P: Packet>(framed: &mut Framed<TcpStream, Codec>, packet: P) {
    let mut writer = Writer::new();
    if let Err(e) = packet.encode(&mut writer) {
        eprintln!("Failed to encode packet: {e}");
        return;
    }

    let data = writer.data;
    let mut out = Writer::new();
    out.write_varint(data.len() as i32);

    let mut buf = BytesMut::new();
    buf.extend_from_slice(&out.data);
    buf.extend_from_slice(&data);

    println!("DEBUG send_packet bytes: {:?}", buf.to_vec());

    if let Err(e) = framed.get_mut().write_all(&buf).await {
        eprintln!("Failed to send packet: {e}");
    }
}

pub async fn process(socket: TcpStream, registry: Arc<PacketRegistry>) {
    let codec = Codec {
        registry: registry.clone(),
        state: EnumProtocol::Handshaking,
    };
    let mut framed = Framed::new(socket, codec);

    while let Some(result) = framed.next().await {
        match result {
            Ok(packet) => {
                println!("INFO: Received packet: {:?}", packet);

                if let Some(handshake) = packet.as_any().downcast_ref::<PacketHandshake>() {
                    println!(
                        "Handshake received. Sending Status {:?}",
                        handshake.requested_protocol
                    );
                    framed.codec_mut().state = handshake.requested_protocol.clone();
                    continue;
                }
                if packet.as_any().downcast_ref::<Request>().is_some() {
                    println!("Status request → sending response");
                    send_packet(
                        &mut framed,
                        Response::new("Coral Rust Minecraft Server\nTest Server", 0, 20),
                    )
                    .await;
                    continue;
                }

                if let Some(ping) = packet.as_any().downcast_ref::<Ping>() {
                    println!("Ping → sending pong ({})", ping.time);
                    send_packet(&mut framed, Pong { time: ping.time }).await;
                    continue;
                }

                println!("WARN: Unhandled packet: {:?}", packet);
                continue;
            }
            Err(e) => {
                eprintln!("Error processing packet: {:?}", e);
                continue;
            }
        }
    }
}
