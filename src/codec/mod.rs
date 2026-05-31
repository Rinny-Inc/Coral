use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering::Relaxed;
use std::time::Duration;
use std::vec;

use bytes::{Buf, Bytes, BytesMut};
use futures::SinkExt;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::time::interval;
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Encoder, Framed};
use uuid::Uuid;

use crate::config::Config;
use crate::protocol::auth::{AuthProfile, authenticate, compute_server_hash};
use crate::protocol::encryption::{
    Encryption, decrypt_rsa, generate_rsa_key, generate_verify_token,
};
use crate::protocol::packets::handshake::keepalive::KeepAlive;
use crate::protocol::packets::login::disconnect::{LoginDisconnect, PlayDisconnect};
use crate::protocol::packets::login::{EncryptionRequest, EncryptionResponse};
use crate::protocol::packets::play::chat::{ChatMessage, ChatMessageOut};
use crate::protocol::packets::play::player_list::{PlayerListItem, PlayerListItem17};
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
use crate::world::chunk::ChunkData;
use crate::world::time::TimeUpdate;
pub struct Codec {
    pub registry: Arc<PacketRegistry>,
    pub state: EnumProtocol,
    pub encryption: Option<Encryption>,
    decrypted_buf: BytesMut,
}

impl Decoder for Codec {
    type Item = Box<dyn Packet>;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // decrypt any new incoming bytes into our persistent decrypted buffer
        if let Some(enc) = &mut self.encryption {
            if !src.is_empty() {
                let mut new_bytes = src.to_vec();
                enc.decrypt(&mut new_bytes);
                self.decrypted_buf.extend_from_slice(&new_bytes);
                src.clear();
            }
        } else {
            if !src.is_empty() {
                self.decrypted_buf.extend_from_slice(src);
                src.clear();
            }
        }

        if self.decrypted_buf.is_empty() {
            return Ok(None);
        }

        // parse length from decrypted buffer
        let mut reader = Reader::new(&self.decrypted_buf);
        let length = reader.read_varint() as usize;
        let length_prefix_size = reader.position;

        if self.decrypted_buf.len() < length_prefix_size + length {
            return Ok(None);
        }

        self.decrypted_buf.advance(length_prefix_size);
        let payload = self.decrypted_buf.split_to(length);

        let mut bytes = Bytes::from(payload.to_vec());

        let id = {
            let mut inner_reader = Reader::new(&bytes);
            let id = inner_reader.read_varint();
            bytes.advance(inner_reader.position);
            id
        };

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

        let mut frame = length_writer.data;
        frame.extend_from_slice(&data);

        if let Some(enc) = &mut self.encryption {
            enc.encrypt(&mut frame);
        }

        dst.reserve(frame.len()); // reserve memory
        dst.extend_from_slice(&frame);
        Ok(())
    }
}

async fn send_packet<P: Packet + 'static>(framed: &mut Framed<TcpStream, Codec>, packet: P) {
    let boxed_packet: Box<dyn Packet> = Box::new(packet);

    if let Err(e) = framed.send(boxed_packet).await {
        eprintln!("Failed to cleanly dispatch packet frame: {e}");
    }
}

const ALLOWED_PROTOCOLS: &[i32] = &[/*5,*/ 47];
pub async fn process(
    socket: TcpStream,
    registry: Arc<PacketRegistry>,
    online: Arc<AtomicU32>,
    server_icon: Arc<Option<String>>,
    config: Arc<Config>,
    chat_tx: Arc<broadcast::Sender<String>>,
) {
    let codec = Codec {
        registry: registry.clone(),
        state: EnumProtocol::Handshaking,
        encryption: None,
        decrypted_buf: BytesMut::new(),
    };
    let mut chat_rx = chat_tx.subscribe();
    let mut framed = Framed::new(socket, codec);
    let mut client_protocol = 1;
    let mut keep_alive_interval = interval(Duration::from_secs(15)); // 30 seconds is timed out
    let mut keep_alive_id = 0;

    let (private_key, public_key_der) = generate_rsa_key();
    let verify_token = generate_verify_token();
    let mut pending_username: Option<String> = None;

    let mut joined = false;
    let mut player_name: Option<String> = None;

    loop {
        tokio::select! {
            Ok(message) = chat_rx.recv() => {
                if framed.codec().state == EnumProtocol::Play {
                    send_packet(&mut framed, ChatMessageOut::from_json(&message)).await;
                }
            }

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
                            send_packet(
                                &mut framed,
                                Response::new(
                                    &config.server.motd,
                                    online.load(Relaxed),
                                    config.server.max_player,
                                    client_protocol,
                                    server_icon.as_deref()
                                ),
                            )
                            .await;
                            continue;
                        }

                        if let Some(ping) = packet.as_any().downcast_ref::<Ping>() {
                            //println!("Ping → sending pong ({})", ping.time);
                            send_packet(&mut framed, Pong { time: ping.time }).await;
                            continue;
                        }

                        if framed.codec().state == EnumProtocol::Login
                            && !ALLOWED_PROTOCOLS.contains(&client_protocol)
                        {
                            kick(&mut framed, "Unsupported version. Use 1.7.x or 1.8.x").await;
                            break;
                        }

                        if config.server.online_mode {
                            if let Some(login_start) = packet.as_any().downcast_ref::<LoginStart>() {
                                pending_username = Some(login_start.username.clone());

                                send_packet(&mut framed, EncryptionRequest {
                                    server_id: "".to_string(),
                                    public_key: public_key_der.clone(),
                                    verify_token: verify_token.clone(),
                                }).await;
                                continue;
                            }

                            if let Some(enc_resp) = packet.as_any().downcast_ref::<EncryptionResponse>() {
                                let shared_secret = decrypt_rsa(&private_key, &enc_resp.shared_secret);
                                let decrypted_token = decrypt_rsa(&private_key, &enc_resp.verify_token);

                                if decrypted_token != verify_token {
                                    kick(&mut framed, "Encryption Error!").await;
                                    break;
                                }

                                let username = match pending_username.take() {
                                    Some(u) => u,
                                    None => break
                                };

                                let server_hash = compute_server_hash("", &shared_secret, &public_key_der);

                                let profile = match authenticate(&username, &server_hash).await {
                                    Some(p) => p,
                                    None => {
                                        kick(&mut framed, "Failed to verify username!").await;
                                        break
                                    }
                                };

                                framed.codec_mut().encryption = Some(Encryption::new(&shared_secret));

                                make_player_join(&mut framed, profile, client_protocol, config.server.max_player as u8).await;
                                player_name = Some(username.clone());
                                online.fetch_add(1, Relaxed);
                                println!(
                                    "Player joined: {}, online: {}",
                                    username,
                                    online.load(Relaxed)
                                );
                                joined = true;
                                continue;
                            }
                        } else {
                                if let Some(login_start) = packet.as_any().downcast_ref::<LoginStart>() {

                                let uuid = Uuid::new_v3(
                                    &Uuid::NAMESPACE_DNS,
                                    format!("OfflinePlayer:{}", login_start.username).as_bytes(),
                                );

                                let profile = AuthProfile {
                                    uuid: uuid.to_string(),
                                    username: login_start.username.clone(),
                                    properties: vec![]
                                };

                                make_player_join(&mut framed, profile, client_protocol, config.server.max_player as u8).await;
                                player_name = Some(login_start.username.clone());
                                online.fetch_add(1, Relaxed);
                                println!(
                                    "Player joined: {}, online: {}",
                                    login_start.username,
                                    online.load(Relaxed)
                                );
                                joined = true;
                                continue;
                            }
                        }

                        if let Some(ka) = packet.as_any().downcast_ref::<KeepAlive>() {
                            println!("Keep Alive response: {}", ka.id);
                            continue;
                        }

                        if let Some(chat) = packet.as_any().downcast_ref::<ChatMessage>() {
                            if let Some(ref name) = player_name {
                                if chat.message.len() > 100 { // FIXME: check the correct length
                                    continue;
                                }

                                let formatted = config.chat.format.replace("{username}", name).replace("{message}", &chat.message);

                                println!("[CHAT] {}", formatted);

                                let json = format!(
                                    "{{\"text\":\"{}\"}}",
                                    formatted.replace('"', "\\\"")
                                );
                                if chat_tx.send(json).is_err() {
                                    eprintln!("Failed to send json to Chat Sender!");
                                };
                            }
                            continue;
                        }

                        println!("WARN: Unhandled packet: {:?}", packet);
                        continue;
                    }
                    Err(e) => {
                        eprintln!("Error processing packet: {:?}", e);
                        break;
                    }
                }
            }
        }
    }
    if joined {
        online.fetch_sub(1, Relaxed);
        println!("Player left, online: {}", online.load(Relaxed));
    }
}

async fn kick(framed: &mut Framed<TcpStream, Codec>, reason: &str) {
    match framed.codec().state {
        EnumProtocol::Login => {
            send_packet(framed, LoginDisconnect::new(reason)).await;
        }
        EnumProtocol::Play => {
            send_packet(framed, PlayDisconnect::new(reason)).await;
        }
        _ => {}
    }
}

async fn make_player_join(
    framed: &mut Framed<TcpStream, Codec>,
    profile: AuthProfile,
    client_protocol: i32,
    max_players: u8,
) {
    let uuid = Uuid::parse_str(&profile.uuid).unwrap_or_else(|_| Uuid::new_v4());

    send_packet(
        framed,
        LoginSuccess {
            uuid,
            username: profile.username.clone(),
        },
    )
    .await;

    framed.codec_mut().state = handshake::EnumProtocol::Play;

    send_packet(
        framed,
        JoinGame {
            entity_id: 1,
            gamemode: 0,
            dimension: 0,
            difficulty: 1,
            max_player: max_players,
            level_type: "default".to_string(),
            reduced_debug_info: false,
        },
    )
    .await;

    if client_protocol == 47 {
        send_packet(
            framed,
            PlayerListItem {
                uuid,
                username: profile.username.clone(),
                properties: profile.properties.clone(),
                gamemode: 0,
                ping: 20,
            },
        )
        .await;
    } else {
        send_packet(
            framed,
            PlayerListItem17 {
                username: profile.username.clone(),
                online: true,
                ping: 20,
            },
        )
        .await;
    }

    send_packet(
        framed,
        TimeUpdate {
            world_age: 0,
            time_of_day: 6000,
        },
    )
    .await;

    if client_protocol == 47 {
        send_packet(framed, SpawnPosition { x: 0, y: 64, z: 0 }).await;
    } else {
        send_packet(framed, SpawnPosition17 { x: 0, y: 64, z: 0 }).await;
    }

    send_packet(
        framed,
        PlayerAbilities {
            flags: 0x00,
            fly_speed: 0.05,
            walk_speed: 0.1,
        },
    )
    .await;

    for cx in -2i32..=2 {
        for cz in -2i32..=2 {
            send_packet(
                framed,
                ChunkData {
                    chunk_x: cx,
                    chunk_z: cz,
                    client_protocol,
                },
            )
            .await;
        }
    }

    send_packet(
        framed,
        PlayerPositionAndLook {
            x: 0.5,
            y: 4.5,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
        },
    )
    .await;
}
