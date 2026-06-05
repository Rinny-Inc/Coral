use std::sync::Arc;
use std::time::Duration;
use std::vec;

use bytes::{Buf, Bytes, BytesMut};
use futures::SinkExt;
use rsa::RsaPrivateKey;
use tokio::net::TcpStream;
use tokio::sync::{RwLock, broadcast};
use tokio::time::interval;
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Encoder, Framed};
use uuid::Uuid;

use crate::command::{CommandContext, CommandDispatcher, CommandResult};
use crate::config::Config;
use crate::protocol::auth::{AuthProfile, authenticate, compute_server_hash};
use crate::protocol::encryption::{Encryption, decrypt_rsa, generate_verify_token};
use crate::protocol::packets::handshake::keepalive::KeepAlive;
use crate::protocol::packets::login::disconnect::{LoginDisconnect, PlayDisconnect};
use crate::protocol::packets::login::{EncryptionRequest, EncryptionResponse};
use crate::protocol::packets::play::PluginMessage;
use crate::protocol::packets::play::block::{
    BlockChange, HeldItemChange, PlayerBlockPlacement, PlayerDig,
};
use crate::protocol::packets::play::chat::{
    ChatMessage, ChatMessageOut, TabComplete, TabCompleteResponse,
};
use crate::protocol::packets::play::entity::{
    ArmAnimation, DestroyEntities, EntityAction, EntityAnimation, EntityHeadLook, EntityMetadata,
    EntityTeleport, EntityVelocity, SpawnPlayer, UseEntity,
};
use crate::protocol::packets::play::game::{ChangeGameState, ClientStatus, Respawn, UpdateHealth};
use crate::protocol::packets::play::inventory::{ClickWindow, CloseWindow, ConfirmTransaction};
use crate::protocol::packets::play::movement::{
    PlayerLook, PlayerOnGround, PlayerPosition, PlayerPositionAndLookIn,
};
use crate::protocol::packets::play::player_list::{
    BulkUpdateLatency, PlayerListItem, PlayerListItem17, UpdateLatency,
};
use crate::protocol::packets::{PacketIn, PacketOut};
use crate::protocol::{
    packets::{
        PacketKey, PacketRegistry,
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
use crate::server::ops::OpsFile;
use crate::server::player::Player;
use crate::server::registry::{PlayerRegistry, next_entity_id};
use crate::world::blocks::{Block, WorldBlocks};
use crate::world::chunk::ChunkData;
use crate::world::time::TimeUpdate;
use crate::{
    AnimationUpdate, BlockUpdate, DamageEvent, GamemodeUpdate, JoinLeave, MetadataUpdate,
    PingUpdate, PositionUpdate,
};
pub struct Codec {
    pub registry: Arc<PacketRegistry>,
    pub state: EnumProtocol,
    pub encryption: Option<Encryption>,
    decrypted_buf: BytesMut,
}

impl Decoder for Codec {
    type Item = Box<dyn PacketIn>;
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

impl Encoder<Box<dyn PacketOut>> for Codec {
    type Error = std::io::Error;

    fn encode(&mut self, item: Box<dyn PacketOut>, dst: &mut BytesMut) -> Result<(), Self::Error> {
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

async fn send_packet<P: PacketOut + 'static>(framed: &mut Framed<TcpStream, Codec>, packet: P) {
    let boxed_packet: Box<dyn PacketOut> = Box::new(packet);

    if let Err(e) = framed.send(boxed_packet).await {
        eprintln!("Failed to cleanly dispatch packet frame: {e}");
    }
}

const ALLOWED_PROTOCOLS: &[i32] = &[/*5,*/ 47];

struct PlayerState {
    uuid: Option<Uuid>,
    entity_id: i32,
    gamemode: u8,
    held_item: i16,
    health: f32,
    food: i32,
    food_saturation: f32,
    is_dead: bool,
    is_sneaking: bool,
    is_sprinting: bool,
    latency_ms: u32,
    food_timer: u8,
    name: Option<String>,
    pending_username: Option<String>,
    keep_alive_count: i32,
    last_sent_keep_alive: Option<(i32, std::time::Instant)>,
}
impl PlayerState {
    fn new(default_gamemode: u8) -> Self {
        Self {
            uuid: None,
            entity_id: 0,
            gamemode: default_gamemode,
            held_item: -1,
            health: 20.0,
            food: 20,
            food_saturation: 5.0,
            is_dead: false,
            is_sneaking: false,
            is_sprinting: false,
            latency_ms: 0,
            food_timer: 0,
            name: None,
            pending_username: None,
            keep_alive_count: 0,
            last_sent_keep_alive: None,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn process(
    socket: TcpStream,
    registry: Arc<PacketRegistry>,
    server_icon: Arc<Option<String>>,
    config: Arc<Config>,
    dispatcher: Arc<CommandDispatcher>,
    chat_tx: Arc<broadcast::Sender<String>>,
    join_tx: Arc<broadcast::Sender<JoinLeave>>,
    pos_tx: Arc<broadcast::Sender<PositionUpdate>>,
    gm_tx: Arc<broadcast::Sender<GamemodeUpdate>>,
    ping_tx: Arc<broadcast::Sender<PingUpdate>>,
    block_tx: Arc<broadcast::Sender<BlockUpdate>>,
    anim_tx: Arc<broadcast::Sender<AnimationUpdate>>,
    meta_tx: Arc<broadcast::Sender<MetadataUpdate>>,
    dmg_tx: Arc<broadcast::Sender<DamageEvent>>,
    world_blocks: Arc<WorldBlocks>,
    player_registry: Arc<PlayerRegistry>,
    private_key: Arc<RsaPrivateKey>,
    public_key_der: Arc<Vec<u8>>,
    ops: Arc<RwLock<OpsFile>>,
) {
    let codec = Codec {
        registry: registry.clone(),
        state: EnumProtocol::Handshaking,
        encryption: None,
        decrypted_buf: BytesMut::new(),
    };
    let mut chat_rx = chat_tx.subscribe();
    let mut join_rx = join_tx.subscribe();
    let mut pos_rx = pos_tx.subscribe();
    let mut gm_rx = gm_tx.subscribe();
    let mut ping_rx = ping_tx.subscribe();
    let mut block_rx = block_tx.subscribe();
    let mut anim_rx = anim_tx.subscribe();
    let mut meta_rx = meta_tx.subscribe();
    let mut dmg_rx = dmg_tx.subscribe();

    let mut state = PlayerState::new(config.server.default_gamemode);

    let mut framed = Framed::new(socket, codec);
    let mut client_protocol = 1;
    let mut keep_alive_interval = interval(Duration::from_secs(15)); // 30 seconds is timed out

    let verify_token = generate_verify_token();

    loop {
        tokio::select! {
            _ = keep_alive_interval.tick() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                state.keep_alive_count += 1;
                state.last_sent_keep_alive = Some((state.keep_alive_count, std::time::Instant::now()));
                send_packet(&mut framed, KeepAlive { id: state.keep_alive_count }).await;
            }

            Ok((uuid, eid, x, y, z, yaw, pitch, on_ground)) = pos_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                if Some(uuid) == state.uuid {
                    continue;
                }
                let yaw_byte = ((yaw / 360.0 * 256.0) as i32).rem_euclid(256) as u8;
                let pitch_byte = ((pitch / 360.0 * 256.0) as i32).rem_euclid(256) as u8;

                send_packet(&mut framed, EntityTeleport {
                    entity_id: eid,
                    x, y, z,
                    yaw: yaw_byte, pitch: pitch_byte,
                    on_ground
                }).await;
                send_packet(&mut framed, EntityHeadLook {
                    entity_id: eid,
                    head_yaw: yaw_byte
                }).await;
            }

            Ok((eid, anim)) = anim_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                if eid == state.entity_id {
                    continue;
                }
                send_packet(&mut framed, EntityAnimation {
                    entity_id: eid,
                    animation: anim
                }).await;
            }
            Ok((eid, flags)) = meta_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                if eid == state.entity_id {
                    continue;
                }
                send_packet(&mut framed, EntityMetadata {
                    entity_id: eid,
                    flags
                }).await;
            }
            Ok((x, y, z, block_id, metadata)) = block_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                send_packet(&mut framed, BlockChange {
                    x, y, z,
                    block_id,
                    block_metadata: metadata
                }).await;
            }
            Ok((player, join_event)) = join_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                if Some(player.uuid) == state.uuid {
                    continue;
                }
                if !join_event {
                    send_packet(&mut framed, DestroyEntities {
                        entity_ids: vec![player.entity_id]
                    }).await;
                } else {
                    send_packet(&mut framed, SpawnPlayer {
                        entity_id: player.entity_id,
                        uuid: player.uuid,
                        username: player.username.clone(),
                        x: player.x,
                        y: player.y,
                        z: player.z,
                        yaw: 90.0,
                        pitch: 0.0,
                        current_item: 0
                    }).await;
                }
            }
            Ok((uuid, health, food, food_saturation, attacker_eid)) = dmg_rx.recv() => {
                if Some(uuid) != state.uuid {
                    continue;
                }
                state.health = health;
                state.food = food;
                state.food_saturation = food_saturation;
                state.is_dead = health <= 0.0;

                send_packet(&mut framed, UpdateHealth {
                    health,
                    food,
                    food_saturation,
                }).await;

                if health > 0.0
                    && let Some(uuid_val) = state.uuid
                    && let Some(me) = player_registry.get(&uuid_val).await
                {
                    let players = player_registry.get_all().await;
                    if let Some(attacker) = players.iter().find(|p| p.entity_id == attacker_eid) {
                        let dx = me.x - attacker.x;
                        let dz = me.z - attacker.z;
                        let len = (dx * dx + dz * dz).sqrt().max(0.001);
                        let knockback = 0.4;
                        let vx = (dx / len * knockback * 8000.0) as i16;
                        let vz = (dz / len * knockback * 8000.0) as i16;
                        let vy = 2000i16;

                        send_packet(&mut framed, EntityVelocity {
                            entity_id: state.entity_id,
                            vx,
                            vy,
                            vz
                        }).await;
                    }
                }
            }
            Ok(message) = chat_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                send_packet(&mut framed, ChatMessageOut::from_json(&message)).await;
            }
            Ok((uuid, ping)) = ping_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                send_packet(&mut framed, UpdateLatency {
                    uuid,
                    ping: ping as i32
                }).await;
            }
            Ok((uuid, gamemode)) = gm_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                if let Some(uuid) = state.uuid {
                    state.gamemode = gamemode;
                    send_packet(&mut framed, ChangeGameState::set_gamemode(gamemode)).await;

                    let (flags, fly_speed, walk_speed) = match gamemode {
                        1 => (0x01 | 0x02 | 0x04 | 0x08, 0.5, 0.1),
                        3 => (0x01 | 0x02 | 0x04, 0.05, 0.1),
                        _ => (0x00, 0.5, 0.1),
                    };
                    send_packet(&mut framed, PlayerAbilities {
                        flags,
                        fly_speed,
                        walk_speed
                    }).await;
                }
            }

            result = framed.next() => {
                let Some(result) = result else { break };
                match result {
                    Ok(packet) => {
                        //println!("INFO: Received packet: {:?}", packet);

                        if let Some(handshake) = packet.as_any().downcast_ref::<PacketHandshake>() {
                            client_protocol = handshake.protocol_version;
                            framed.codec_mut().state = handshake.requested_protocol.clone();
                            continue;
                        }
                        if packet.as_any().downcast_ref::<Request>().is_some() {
                            let players = player_registry.get_all().await;
                            let sample: Vec<(&str, String)> = players.iter().take(config.server.player_sample_amount as usize).map(|p| (p.username.as_str(), p.uuid.hyphenated().to_string())).collect();
                            let sample_refs: Vec<(&str, &str)> = sample.iter().map(|(name, uuid)| (*name, uuid.as_str())).collect();

                            let server_protocol = if ALLOWED_PROTOCOLS.contains(&client_protocol) {
                                client_protocol
                            } else {
                                -1
                            };

                            send_packet(
                                &mut framed,
                                Response::new(
                                    &config.server.motd,
                                    player_registry.get_online_count().await,
                                    config.server.max_player,
                                    server_protocol,
                                    server_icon.as_deref(),
                                    &sample_refs
                                ),
                            )
                            .await;
                            continue;
                        }

                        if let Some(ping) = packet.as_any().downcast_ref::<Ping>() {
                            send_packet(&mut framed, Pong { time: ping.time }).await;
                            continue;
                        }

                        // TODO: connection throttled
                        if !ALLOWED_PROTOCOLS.contains(&client_protocol) {
                            kick(&mut framed, "Unsupported version. Use 1.7.10 or 1.8.9").await;
                            break;
                        }
                        if player_registry.get_online_count().await > config.server.max_player {
                            kick(&mut framed, "Server is full!").await;
                            break;
                        }
                        // TODO: is banned
                        if config.server.whitelisted { // check if the uuid is whitelisted
                            // TODO
                        }
                        // TODO: login from another location


                        if config.server.online_mode {
                            if let Some(login_start) = packet.as_any().downcast_ref::<LoginStart>() {
                                state.pending_username = Some(login_start.username.clone());

                                send_packet(&mut framed, EncryptionRequest {
                                    server_id: "".to_string(),
                                    public_key: public_key_der.to_vec(),
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
                                let username = match state.pending_username.take() {
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

                                let result = make_player_join(&mut framed, profile, client_protocol, config.server.max_player as u8, state.latency_ms, &player_registry, &join_tx, &chat_tx, &config).await;
                                // THIS IS RIDICULOUS
                                state.uuid = Some(result.uuid);
                                state.entity_id = result.entity_id;
                                state.name = Some(result.player_name);
                                state.gamemode = result.gamemode;
                                keep_alive_interval.reset();
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

                                let result = make_player_join(&mut framed, profile, client_protocol, config.server.max_player as u8, state.latency_ms, &player_registry, &join_tx, &chat_tx, &config).await;
                                // SO IT IS
                                state.uuid = Some(result.uuid);
                                state.entity_id = result.entity_id;
                                state.name = Some(result.player_name);
                                state.gamemode = result.gamemode;
                                keep_alive_interval.reset();
                                continue;
                            }
                        };

                        if let Some(ka) = packet.as_any().downcast_ref::<KeepAlive>() {
                            if let Some((sent_id, sent_time)) = state.last_sent_keep_alive.take()
                                && ka.id == sent_id
                            {
                                state.latency_ms = sent_time.elapsed().as_millis() as u32;
                                if let Some(uuid) = state.uuid {
                                    player_registry.update_latency(uuid, state.latency_ms).await;
                                    ping_tx.send((uuid, state.latency_ms)).ok();
                                }
                            }
                            continue;
                        }

                        if let Some(held) = packet.as_any().downcast_ref::<HeldItemChange>() {
                            let slot = held.slot as u8;
                            if let Some(uuid) = state.uuid {
                                player_registry.update_held_slot(uuid, slot).await;
                            }
                            state.held_item = -1;
                            continue;
                        }

                        if let Some(dig) = packet.as_any().downcast_ref::<PlayerDig>() {
                            match dig.status {
                                0 if state.gamemode == 1 => {
                                    world_blocks.set(dig.x, dig.y, dig.z, Block::air()).await;
                                    block_tx.send((dig.x, dig.y as i32, dig.z, 0, 0)).ok();
                                }
                                2 if state.gamemode == 0 => {
                                    world_blocks.set(dig.x, dig.y, dig.z, Block::air()).await;
                                    block_tx.send((dig.x, dig.y as i32, dig.z, 0, 0)).ok();
                                }
                                _ => {}
                            }
                            continue;
                        }

                        if let Some(place) = packet.as_any().downcast_ref::<PlayerBlockPlacement>() {
                            if place.held_item_id == -1 || place.face == 255 {
                                continue;
                            }
                            if state.gamemode == 2 || state.gamemode == 3 {
                                continue;
                            }
                            let (tx, ty, tz): (i32, i32, i32) = match place.face {
                                0 => (place.x, place.y as i32 - 1, place.z),
                                1 => (place.x, place.y as i32 + 1, place.z),
                                2 => (place.x, place.y as i32, place.z - 1),
                                3 => (place.x, place.y as i32, place.z + 1),
                                4 => (place.x - 1, place.y as i32, place.z),
                                5 => (place.x + 1, place.y as i32, place.z),
                                _ => continue
                            };

                            if !(0..=255).contains(&ty) {
                                continue;
                            }

                            let block_id = place.held_item_id as i32;
                            if block_id <= 0 || block_id > 255 {
                                continue;
                            }

                            state.held_item = place.held_item_id;
                            if let Some(uuid) = state.uuid {
                                player_registry.update_held_item(uuid, state.held_item).await;
                            }

                            world_blocks.set(tx, ty as u8, tz, Block::new(block_id as u8, 0)).await;
                            block_tx.send((tx, ty, tz, block_id, 0)).ok();
                            continue;
                        }

                        if let Some(arm) = packet.as_any().downcast_ref::<ArmAnimation>() {
                            anim_tx.send((state.entity_id, 0)).ok();
                            continue;
                        }

                        if let Some(chat) = packet.as_any().downcast_ref::<ChatMessage>() {
                            if chat.message.starts_with('/') {
                                let args: Vec<String> = chat.message[1..]
                                                            .split_whitespace()
                                                            .map(|s| s.to_string())
                                                            .collect();
                                if args.is_empty() {
                                    continue;
                                }

                                let ctx = CommandContext {
                                    sender: state.name.clone().unwrap_or_default(),
                                    args
                                };

                                match dispatcher.dispatch(ctx).await {
                                    CommandResult::Success(msg) => {
                                        send_packet(&mut framed, ChatMessageOut::from_json(
                                            &format!("{{\"text\":\"{}\"}}", msg.replace('"', "\\\""))
                                        )).await;
                                    }
                                    CommandResult::Error(msg) => {
                                        send_packet(&mut framed, ChatMessageOut::from_json(
                                            &format!("{{\"text\":\"{}\",\"color\":\"red\"}}", msg.replace('"', "\\\""))
                                        )).await;
                                    }
                                    CommandResult::Broadcast(msg) => {
                                        chat_tx.send(format!("{{\"text\":\"{}\"}}", msg.replace('"', "\\\""))).ok();
                                    }
                                    CommandResult::None => {}
                                }
                                continue;
                            }
                            if let Some(ref name) = state.name {
                                if chat.message.len() > 100 { // FIXME: check the correct length
                                    continue;
                                }

                                let formatted = config.chat.format.replace("{username}", name).replace("{message}", &chat.message);

                                println!("[CHAT] {}", formatted);

                                let json = format!(
                                    "{{\"text\":\"{}\"}}",
                                    formatted.replace('"', "\\\"")
                                );
                                chat_tx.send(json).ok();
                            }
                            continue;
                        }
                        if let Some(tab) = packet.as_any().downcast_ref::<TabComplete>() {
                            let text = &tab.text;
                            let matches: Vec<String> = if let Some(partial) = text.strip_prefix('/') {
                                dispatcher.completions(partial).await
                            } else {
                                player_registry.get_all().await
                                    .iter()
                                    .filter(|p| p.username.to_lowercase().starts_with(&text.to_lowercase()))
                                    .map(|p| p.username.clone())
                                    .collect()
                            };

                            send_packet(&mut framed, TabCompleteResponse {
                                matches
                            }).await;
                            continue;
                        }

                        if let Some(pos) = packet.as_any().downcast_ref::<PlayerPosition>() {
                            if let Some(uuid) = state.uuid
                                && let Some(p) = player_registry.get(&uuid).await
                            {
                                if pos.on_ground && !p.on_ground && state.gamemode == 0 {
                                    let fall_distance = p.y - pos.y;
                                    if fall_distance > 3.0 {
                                        let damage = (fall_distance - 3.0) as f32;
                                        damage_player(
                                            &mut framed,
                                            &mut state.health,
                                            &mut state.food,
                                            &mut state.food_saturation,
                                            &mut state.is_dead,
                                            damage,
                                            &player_registry,
                                            uuid
                                        ).await;
                                    }
                                }
                                let moved = (pos.x - p.x).abs() > 0.01
                                    || (pos.y - p.y).abs() > 0.01
                                    || (pos.z - p.z).abs() > 0.01;

                                if moved {
                                    player_registry.update_position(&uuid, pos.x, pos.y, pos.z, 90.0, 0.0, pos.on_ground).await;
                                    pos_tx.send((uuid, state.entity_id, pos.x, pos.y, pos.z, 90.0, 0.0, pos.on_ground)).ok();
                                }
                            }
                            continue;
                        }

                        if let Some(look) = packet.as_any().downcast_ref::<PlayerLook>() {
                            if let Some(uuid) = state.uuid
                                && let Some(p) = player_registry.get(&uuid).await
                            {
                                player_registry.update_position(&uuid, p.x, p.y, p.z, look.yaw, look.pitch, look.on_ground).await;
                                pos_tx.send((uuid, state.entity_id, p.x, p.y, p.z, look.yaw, look.pitch, look.on_ground)).ok();
                            }
                            continue;
                        }

                        if let Some(close) = packet.as_any().downcast_ref::<CloseWindow>() {
                            // TODO
                            continue;
                        }

                        if let Some(click) = packet.as_any().downcast_ref::<ClickWindow>() {
                            send_packet(&mut framed, ConfirmTransaction {
                                window_id: click.window_id,
                                action_number: click.action_number,
                                accepted: true
                            }).await;
                            continue;
                        }

                        if let Some(status) = packet.as_any().downcast_ref::<ClientStatus>() {
                            if status.action == 0 && state.is_dead {
                                state.health = 20.0;
                                state.food = 20;
                                state.food_saturation = 5.0;
                                state.is_dead = false;

                                if let Some(uuid) = state.uuid {
                                    player_registry.update_health(uuid, state.health, state.food, state.food_saturation).await;
                                }

                                send_packet(&mut framed, Respawn {
                                    dimension: 0,
                                    difficulty: config.world.difficulty,
                                    gamemode: state.gamemode,
                                    level_type: "flat".to_string()
                                }).await;

                                send_chunks(&mut framed, client_protocol).await;

                                send_packet(&mut framed, PlayerPositionAndLook {
                                    x: 0.5,
                                    y: 5.0,
                                    z: 0.5,
                                    yaw: 90.0,
                                    pitch: 0.0,
                                    on_ground: false
                                }).await;

                                send_packet(&mut framed, UpdateHealth {
                                    health: state.health,
                                    food: state.food,
                                    food_saturation: state.food_saturation
                                }).await;
                            }
                            continue;
                        }

                        if let Some(action) = packet.as_any().downcast_ref::<EntityAction>() {
                            if let Some(uuid) = state.uuid {
                                let update = match action.action {
                                    0 => Some((true, true)),   // sneaking on
                                    1 => Some((true, false)),  // sneaking off
                                    3 => Some((false, true)),  // sprinting on
                                    4 => Some((false, false)), // sprinting off
                                    _ => None,
                                };

                                if let Some((sneaking, value)) = update {
                                    if sneaking {
                                        state.is_sneaking = value;
                                        player_registry.update_sneaking(uuid, value).await;
                                    } else {
                                        state.is_sprinting = value;
                                        player_registry.update_sprinting(uuid, value).await;
                                    }

                                    if let Some(player) = player_registry.get(&uuid).await {
                                        meta_tx.send((state.entity_id, player.entity_flags())).ok();
                                    }
                                }
                            }
                            continue;
                        }

                        if let Some(use_entity) = packet.as_any().downcast_ref::<UseEntity>() {
                            if use_entity.action == 1 && state.gamemode != 3 {
                                let players = player_registry.get_all().await;
                                if let Some(target) = players.iter().find(|p| p.entity_id == use_entity.target_entity_id) {
                                    if target.is_dead {
                                        continue;
                                    }
                                    let new_health = (target.health - 0.0).max(0.0);
                                    player_registry.update_health(target.uuid, new_health, target.food, target.food_saturation).await;

                                    dmg_tx.send((target.uuid, new_health, target.food, target.food_saturation, state.entity_id)).ok();
                                    anim_tx.send((use_entity.target_entity_id, 1)).ok();
                                }
                            }
                            continue;
                        }

                        if let Some(pos_look) = packet.as_any().downcast_ref::<PlayerPositionAndLookIn>() {
                            if let Some(uuid) = state.uuid {
                                player_registry.update_position(&uuid, pos_look.x, pos_look.y, pos_look.z, pos_look.yaw, pos_look.pitch, pos_look.on_ground).await;
                                pos_tx.send((uuid, state.entity_id, pos_look.x, pos_look.y, pos_look.z, pos_look.yaw, pos_look.pitch, pos_look.on_ground)).ok();
                            }
                            continue;
                        }

                        if let Some(og) = packet.as_any().downcast_ref::<PlayerOnGround>() {
                            if let Some(uuid) = state.uuid
                                && let Some(p) = player_registry.get(&uuid).await
                            {
                                player_registry.update_position(
                                    &uuid,
                                    p.x, p.y, p.z,
                                    p.yaw, p.pitch,
                                    og.on_ground,
                                ).await;
                            }
                            continue;
                        }

                        if let Some(plugin) = packet.as_any().downcast_ref::<PluginMessage>() {
                            if plugin.channel == "MC|Brand" {
                                send_packet(&mut framed, PluginMessage::brand("Coral")).await;
                            }
                            continue;
                        }

                        println!("WARN: Unhandled packet: {:?}", packet);
                        continue;
                    }
                    Err(e) => {
                        match e.kind() {
                            std::io::ErrorKind::TimedOut |
                            std::io::ErrorKind::ConnectionReset |
                            std::io::ErrorKind::ConnectionAborted |
                            std::io::ErrorKind::UnexpectedEof |
                            std::io::ErrorKind::BrokenPipe => {
                                // NO LOG
                            }
                            _ => eprintln!("Error processing packet: {:?}", e),
                        }
                        break;
                    }
                }
            }
        }
    }
    if let (Some(uuid), Some(name)) = (state.uuid, state.name) {
        player_registry.remove(&uuid).await;
        join_tx
            .send((Player::new(state.entity_id, uuid, String::new()), false))
            .ok();
        if player_registry.players.read().await.is_empty() {
            let leave_msg = format!(
                "{{\"text\":\"{} left the game\",\"color\":\"yellow\"}}",
                name
            );
            chat_tx.send(leave_msg).ok();
        }
        println!(
            "{} left the game; Online: {}",
            name,
            player_registry.get_online_count().await
        );
    }
}

#[allow(clippy::too_many_arguments)]
async fn damage_player(
    framed: &mut Framed<TcpStream, Codec>,
    health: &mut f32,
    food: &mut i32,
    food_saturation: &mut f32,
    is_dead: &mut bool,
    amount: f32,
    player_registry: &Arc<PlayerRegistry>,
    uuid: Uuid,
) {
    *health = (*health - amount).max(0.0);

    if let Some(uuid) = Some(uuid) {
        player_registry
            .update_health(uuid, *health, *food, *food_saturation)
            .await;
    }

    send_packet(
        framed,
        UpdateHealth {
            health: *health,
            food: *food,
            food_saturation: *food_saturation,
        },
    )
    .await;

    if *health <= 0.0 {
        *is_dead = true;
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

struct JoinResult {
    uuid: Uuid,
    entity_id: i32,
    player_name: String,
    gamemode: u8,
}

async fn send_chunks(framed: &mut Framed<TcpStream, Codec>, client_protocol: i32) {
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
}

#[allow(clippy::too_many_arguments)]
async fn make_player_join(
    framed: &mut Framed<TcpStream, Codec>,
    profile: AuthProfile,
    client_protocol: i32,
    max_players: u8,
    latency: u32,
    player_registry: &Arc<PlayerRegistry>,
    join_tx: &Arc<broadcast::Sender<JoinLeave>>,
    chat_tx: &Arc<broadcast::Sender<String>>,
    config: &Config,
) -> JoinResult {
    let uuid = Uuid::parse_str(&profile.uuid).unwrap_or_else(|_| Uuid::new_v4());
    let entity_id = next_entity_id();

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
            entity_id,
            gamemode: config.server.default_gamemode,
            dimension: 0,
            difficulty: config.world.difficulty,
            max_player: max_players,
            level_type: "flat".to_string(),
            reduced_debug_info: false,
        },
    )
    .await;

    send_packet(
        framed,
        ChangeGameState::set_gamemode(config.server.default_gamemode),
    )
    .await;

    if client_protocol == 47 {
        send_packet(
            framed,
            PlayerListItem {
                uuid,
                username: profile.username.clone(),
                properties: profile.properties.clone(),
                gamemode: config.server.default_gamemode as i32,
                ping: latency as i32,
            },
        )
        .await;

        let entries: Vec<(Uuid, i32)> = player_registry
            .get_all()
            .await
            .iter()
            .map(|p| (p.uuid, p.latency_ms as i32))
            .collect();

        if !entries.is_empty() {
            send_packet(framed, BulkUpdateLatency { entries }).await;
        }
    } else {
        send_packet(
            framed,
            PlayerListItem17 {
                username: profile.username.clone(),
                online: true,
                ping: latency as i16,
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

    send_chunks(framed, client_protocol).await;

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

    let player = Player::new(entity_id, uuid, profile.username.clone());
    player_registry.add(player.clone()).await;

    let join_msg = format!(
        "{{\"text\":\"{} joined the game\",\"color\":\"yellow\"}}",
        profile.username
    );
    chat_tx.send(join_msg).ok();

    println!(
        "{} joined the game; Online: {}",
        profile.username,
        player_registry.get_online_count().await
    );

    let existing_players = player_registry.get_all().await;
    for p in existing_players {
        if p.uuid == uuid {
            continue;
        }
        send_packet(
            framed,
            SpawnPlayer {
                entity_id: p.entity_id,
                uuid: p.uuid,
                username: p.username.clone(),
                x: p.x,
                y: p.y,
                z: p.z,
                yaw: 90.0,
                pitch: 0.0,
                current_item: 0,
            },
        )
        .await
    }

    send_packet(
        framed,
        UpdateHealth {
            health: 20.0,
            food: 20,
            food_saturation: 5.0,
        },
    )
    .await;

    join_tx.send((player, true)).ok();

    JoinResult {
        uuid,
        entity_id,
        player_name: profile.username,
        gamemode: config.server.default_gamemode,
    }
}
