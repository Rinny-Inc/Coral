use std::sync::Arc;
use std::time::Duration;

use bytes::{Buf, Bytes, BytesMut};
use tokio::net::TcpStream;
use tokio::{io::AsyncWriteExt, time::interval};
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Encoder, Framed};
use uuid::Uuid;

use crate::protocol::packets::handshake::keepalive::KeepAlive;
use crate::protocol::{
    packets::{
        Packet, PacketKey, PacketRegistry,
        handshake::{self, EnumProtocol, PacketHandshake},
        login::{LoginStart, LoginSuccess},
        play::{
            PlayerAbilities, PlayerPositionAndLook, SpawnPosition, SpawnPosition17,
            join_game::JoinGame,
        },
        status::{Ping, Pong, Request, Response},
    },
    reader::Reader,
    writer::Writer,
};
pub struct Codec {
    pub registry: Arc<PacketRegistry>,
    pub state: EnumProtocol,
    online: u32,
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
            None => {
                if self.state == EnumProtocol::Play {
                    println!(
                        "WARN: Ignoring unknown Play packet ID: 0x{:02X} ({})",
                        id, id
                    );
                    Ok(None)
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Unknown packet ID: {}", id),
                    ))
                }
            }
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

const ALLOWED_PROTOCOLS: &[i32] = &[5, 47];
const MAX_PLAYER: u32 = 20;
pub async fn process(socket: TcpStream, registry: Arc<PacketRegistry>) {
    let codec = Codec {
        registry: registry.clone(),
        state: EnumProtocol::Handshaking,
        online: 0,
    };
    let mut framed = Framed::new(socket, codec);
    let mut client_protocol = 1;
    let mut keep_alive_interval = interval(Duration::from_secs(5));
    let mut keep_alive_id = 0;
    loop {
        tokio::select! {
            _ = keep_alive_interval.tick() => {
                if framed.codec().state == EnumProtocol::Play {
                    keep_alive_id += 1;
                    send_packet(&mut framed, KeepAlive { id: keep_alive_id }).await;
                }
            }
            result = framed.next() => {
                let Some(result) = result else { break };
                match result {
                    Ok(packet) => {
                        println!("INFO: Received packet: {:?}", packet);

                        if let Some(handshake) = packet.as_any().downcast_ref::<PacketHandshake>() {
                            //println!("Handshake received. Sending Status {:?}", handshake.requested_protocol);
                            client_protocol = handshake.protocol_version;
                            framed.codec_mut().state = handshake.requested_protocol.clone();
                            continue;
                        }
                        if packet.as_any().downcast_ref::<Request>().is_some() {
                            //println!("Status request → sending response");
                            let online = framed.codec().online;
                            send_packet(
                                &mut framed,
                                Response::new(
                                    "Coral Rust Minecraft Server\nTest Server",
                                    online,
                                    MAX_PLAYER,
                                    client_protocol,
                                ),
                            )
                            .await;
                            continue;
                        }

                        if let Some(ping) = packet.as_any().downcast_ref::<Ping>() {
                            println!("Ping → sending pong ({})", ping.time);
                            send_packet(&mut framed, Pong { time: ping.time }).await;
                            continue;
                        }

                        if framed.codec().state == EnumProtocol::Login
                            && !ALLOWED_PROTOCOLS.contains(&client_protocol)
                        {
                            break;
                        }

                        if let Some(login_start) = packet.as_any().downcast_ref::<LoginStart>() {
                            println!("Login request from {}", login_start.username);

                            let uuid = Uuid::new_v3(
                                &Uuid::NAMESPACE_DNS,
                                format!("OfflinePlayer:{}", login_start.username).as_bytes(),
                            );

                            send_packet(
                                &mut framed,
                                LoginSuccess {
                                    uuid,
                                    username: login_start.username.clone(),
                                },
                            )
                            .await;

                            framed.codec_mut().state = handshake::EnumProtocol::Play;

                            send_packet(
                                &mut framed,
                                JoinGame {
                                    entity_id: 1,
                                    gamemode: 0,
                                    dimension: 0,
                                    difficulty: 1,
                                    max_player: 20,
                                    level_type: "default".to_string(),
                                    reduced_debug_info: false,
                                },
                            )
                            .await;

                            if client_protocol == 47 {
                                send_packet(&mut framed, SpawnPosition { x: 0, y: 64, z: 0 }).await;
                            } else {
                                send_packet(&mut framed, SpawnPosition17 { x: 0, y: 64, z: 0 }).await;
                            }

                            send_packet(
                                &mut framed,
                                PlayerAbilities {
                                    flags: 0x00,
                                    fly_speed: 0.05,
                                    walk_speed: 0.1,
                                },
                            )
                            .await;

                            send_packet(
                                &mut framed,
                                PlayerPositionAndLook {
                                    x: 0.0,
                                    y: 64.0,
                                    z: 0.0,
                                    yaw: 90.0,
                                    pitch: 0.0,
                                    on_ground: true,
                                },
                            )
                            .await;
                            continue;
                        }

                        if let Some(ka) = packet.as_any().downcast_ref::<KeepAlive>() {
                            println!("Keep Alive response: {}", ka.id);
                            continue;
                        }

                        println!("WARN: Unhandled packet: {:?}", packet);
                        break;
                    }
                    Err(e) => {
                        eprintln!("Error processing packet: {:?}", e);
                        break;
                    }
                }
            }
        }
    }
}
