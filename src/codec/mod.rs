use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::vec;

use bytes::{Buf, Bytes, BytesMut};
use futures::SinkExt;
use tokio::net::TcpStream;
use tokio::sync::{RwLock, broadcast};
use tokio::time::interval;
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Encoder, Framed};
use uuid::Uuid;

use crate::command::{CommandContext, CommandResult};
use crate::config::Config;
use crate::protocol::auth::{AuthProfile, authenticate, compute_server_hash};
use crate::protocol::encryption::{Encryption, decrypt_rsa, generate_verify_token};
use crate::protocol::packets::handshake::keepalive::KeepAlive;
use crate::protocol::packets::login::disconnect::{LoginDisconnect, PlayDisconnect};
use crate::protocol::packets::login::{EncryptionRequest, EncryptionResponse};
use crate::protocol::packets::play::block::{
    BlockBreakAnimation, BlockChange, HeldItemChange, PlayerBlockPlacement, PlayerDig,
};
use crate::protocol::packets::play::chat::builder::ChatBuilder;
use crate::protocol::packets::play::chat::builder::ChatColor;
use crate::protocol::packets::play::chat::{
    ChatMessage, ChatMessageOut, TabComplete, TabCompleteResponse,
};
use crate::protocol::packets::play::entity::{
    ArmAnimation, CollectItem, DestroyEntities, EntityAction, EntityAnimation, EntityHeadLook,
    EntityMetadata, EntityTeleport, EntityVelocity, SpawnObject, SpawnPlayer, UseEntity,
};
use crate::protocol::packets::play::game::{
    ChangeGameState, ClientStatus, EntityStatus, Respawn, SetExperience, UpdateHealth,
};
use crate::protocol::packets::play::inventory::{
    ClickWindow, CloseWindow, ConfirmTransaction, CreativeInventoryAction, Inventory, SetSlot,
    Slot, WindowItems,
};
use crate::protocol::packets::play::movement::{
    PlayerLook, PlayerOnGround, PlayerPosition, PlayerPositionAndLookIn,
};
use crate::protocol::packets::play::player_list::{
    BulkUpdateLatency, PlayerListItem, PlayerListItem17, UpdateLatency,
};
use crate::protocol::packets::play::{ClientSettings, PluginMessage};
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
use crate::server::bounding_box::EntityBounds;
use crate::server::entity_tracker::{EntityTracker, TrackedEntity};
use crate::server::player::Player;
use crate::server::registry::{PlayerRegistry, next_entity_id};
use crate::world::blocks::{Block, WorldBlocks};
use crate::world::chunk::ChunkData;
use crate::world::time::TimeUpdate;
use crate::world::weather::WeatherState;
use crate::{JoinLeave, ServerContext};

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
                } else if self.state == EnumProtocol::Status {
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

    if let Err(e) = framed.send(boxed_packet).await
        && !is_normal_disconnect(&e)
    {
        eprintln!("Failed to cleanly dispatch packet frame: {e}");
    }
}

const ALLOWED_PROTOCOLS: &[i32] = &[/*5,*/ 47];

struct PlayerState {
    uuid: Option<Uuid>,
    entity_id: i32,
    gamemode: u8,
    held_item: i16,
    held_slot: u8,
    health: f32,
    food: i32,
    food_saturation: f32,
    food_exhaustion: f32,
    regen_timer: i32,
    is_dead: bool,
    is_sneaking: bool,
    is_sprinting: bool,
    latency_ms: u32,
    name: Option<String>,
    pending_username: Option<String>,
    keep_alive_count: i32,
    last_sent_keep_alive: Option<(i32, std::time::Instant)>,
    inventory: Inventory,
    breaking_block: Option<(i32, i32, i32)>,
    current_weather: WeatherState,
    client_brand: Option<String>,
    skin_parts: u8,
    tick_count: i64,
}
impl PlayerState {
    fn new(default_gamemode: u8) -> Self {
        Self {
            uuid: None,
            entity_id: 0,
            gamemode: default_gamemode,
            held_item: -1,
            held_slot: 0,
            health: 20.0,
            food: 20,
            food_saturation: 5.0,
            food_exhaustion: 0.0,
            regen_timer: 0,
            is_dead: false,
            is_sneaking: false,
            is_sprinting: false,
            latency_ms: 0,
            name: None,
            pending_username: None,
            keep_alive_count: 0,
            last_sent_keep_alive: None,
            inventory: Inventory::new(),
            breaking_block: None,
            current_weather: WeatherState::Clear,
            client_brand: None,
            skin_parts: 0x7F,
            tick_count: 0,
        }
    }
}

pub async fn process(socket: TcpStream, ctx: ServerContext) {
    let ServerContext {
        packet_registry,
        server_icon,
        config,
        dispatcher,
        entity_tracker,
        item_spawn_times,
        item_positions,
        chat_tx,
        join_tx,
        pos_tx,
        gm_tx,
        ping_tx,
        block_tx,
        break_tx,
        anim_tx,
        meta_tx,
        dmg_tx,
        item_tx,
        despawn_tx,
        pickup_tx,
        time_tx,
        weather_tx,
        tick_tx,
        status_tx,
        world_blocks,
        player_registry,
        private_key,
        public_key_der,
        ops,
        whitelist,
        banlist,
    } = ctx;
    let codec = Codec {
        registry: packet_registry,
        state: EnumProtocol::Handshaking,
        encryption: None,
        decrypted_buf: BytesMut::new(),
    };
    let peer_ip = socket.peer_addr().ok();
    let mut chat_rx = chat_tx.subscribe();
    let mut join_rx = join_tx.subscribe();
    let mut pos_rx = pos_tx.subscribe();
    let mut gm_rx = gm_tx.subscribe();
    let mut ping_rx = ping_tx.subscribe();
    let mut block_rx = block_tx.subscribe();
    let mut break_rx = break_tx.subscribe();
    let mut anim_rx = anim_tx.subscribe();
    let mut meta_rx = meta_tx.subscribe();
    let mut dmg_rx = dmg_tx.subscribe();
    let mut item_rx = item_tx.subscribe();
    let mut despawn_rx = despawn_tx.subscribe();
    let mut pickup_rx = pickup_tx.subscribe();
    let mut time_rx = time_tx.subscribe();
    let mut weather_rx = weather_tx.subscribe();
    let mut tick_rx = tick_tx.subscribe();
    let mut status_rx = status_tx.subscribe();

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
            Ok(()) = tick_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play
                    || state.is_dead
                    || state.gamemode != 0
                {
                    continue;
                }

                state.tick_count += 1;

                if config.world.difficulty == 0 {
                    if state.health < 20.0 {
                        state.regen_timer += 1;
                        if state.regen_timer >= 80 {
                            state.regen_timer = 0;
                            state.health = (state.health + 1.0).min(20.0);
                            if let Some(uuid) = state.uuid {
                                player_registry.update_health(uuid, state.health, state.food, state.food_saturation).await;
                            }
                            send_packet(&mut framed, UpdateHealth {
                                health: state.health,
                                food: state.food,
                                food_saturation: state.food_saturation,
                            }).await;
                        }
                    } else {
                        state.regen_timer = 0;
                    }
                    continue;
                }

                if state.food_exhaustion >= 4.0 {
                    state.food_exhaustion -= 4.0;

                    if state.food_saturation > 0.0 {
                        state.food_saturation = (state.food_saturation - 1.0).max(0.0);
                    } else if state.food > 0 {
                        state.food -= 1;
                        state.food_saturation = 0.0;

                        if let Some(uuid) = state.uuid {
                            player_registry.update_health(uuid, state.health, state.food, state.food_saturation).await;
                        }

                        send_packet(&mut framed, UpdateHealth {
                            health: state.health,
                            food: state.food,
                            food_saturation: state.food_saturation
                        }).await;
                    }
                }
                if state.food >= 18 && state.health < 20.0 {
                    state.regen_timer += 1;

                    if state.regen_timer >= 80 {
                        state.regen_timer = 0;
                        state.health = (state.health + 1.0).min(20.0);
                        state.food_exhaustion += 3.0;

                        if let Some(uuid) = state.uuid {
                            player_registry.update_health(uuid, state.health, state.food, state.food_saturation).await;
                        }
                        send_packet(&mut framed, UpdateHealth {
                            health: state.health,
                            food: state.food,
                            food_saturation: state.food_saturation
                        }).await;
                    }
                } else {
                    state.regen_timer = 0;
                }

                if state.food == 0
                    && state.tick_count % 80 == 0
                {
                    let min_health = match config.world.difficulty {
                        1 => 10.0,
                        2 => 1.0,
                        3 => 0.0,
                        _ => 1.0
                    };

                    if state.health > min_health {
                        state.health = (state.health - 1.0).max(min_health);
                        state.is_dead = state.health <= 0.0;

                        if let Some(uuid) = state.uuid {
                            player_registry.update_health(uuid, state.health, state.food, state.food_saturation).await;
                        }

                        send_packet(&mut framed, UpdateHealth {
                            health: state.health,
                            food: state.food,
                            food_saturation: state.food_saturation
                        }).await;
                    }
                }

                if let Some(uuid) = state.uuid
                    && let Some(p) = player_registry.get(&uuid).await
                {
                    let mut items = item_positions.write().await;
                    let mut picked_up = vec![];

                    for (eid, (item_eid, ix, iy, iz, item_id, count, metadata)) in items.iter() {
                        let player_bb = if state.is_sneaking {
                            EntityBounds::player_sneaking()
                        } else {
                            EntityBounds::player()
                        };

                        let item_bb = EntityBounds::item();

                        if player_bb.intersects(
                            p.x, p.y, p.z,
                            &item_bb, *ix, *iy, *iz
                        ){
                        let age = {
                        let spawn_time = item_spawn_times.read().await;
                            spawn_time.get(eid)
                                .map(|t| t.elapsed().as_secs_f32())
                                .unwrap_or(0.0)
                            };

                            if age < 0.5 {
                                continue;
                            }

                            let slot_index = state.inventory.add_item_get_slot(*item_id, *count, *metadata);
                            if let Some(slot) = slot_index {
                                picked_up.push(*eid);

                                send_packet(&mut framed, SetSlot {
                                    window_id: 0,
                                    slot,
                                    item_id: *item_id,
                                    count: *count,
                                    metadata: *metadata
                                }).await;
                            }
                        }
                    }
                    for eid in picked_up {
                        items.remove(&eid);
                        item_spawn_times.write().await.remove(&eid);
                        pickup_tx.send((state.entity_id, uuid, eid)).ok();
                    }
                }
            }
            Ok(weather) = weather_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                state.current_weather = weather.clone();
                send_weather(&mut framed, weather).await;
            }
            Ok(eid) = despawn_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                send_packet(&mut framed, DestroyEntities {
                    entity_ids: vec![eid]
                }).await;
            }
            Ok((eid, status)) = status_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                send_packet(&mut framed, EntityStatus {
                    entity_id: eid,
                    status,
                }).await;
            }
            Ok((collector_eid, collector_uuid, item_eid)) = pickup_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                send_packet(&mut framed, CollectItem {
                    collected_entity_id: item_eid,
                    collector_entity_id: collector_eid,
                }).await;
                send_packet(&mut framed, DestroyEntities {
                    entity_ids: vec![item_eid]
                }).await;
            }
            Ok((world_age, time_of_day)) = time_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                send_packet(&mut framed, TimeUpdate {
                    world_age,
                    time_of_day
                }).await;
            }

            Ok((uuid, eid, x, y, z, yaw, pitch, on_ground)) = pos_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                if Some(uuid) == state.uuid {
                    continue;
                }

                let visible = if let Some(me) = player_registry.get(&state.uuid.unwrap_or_default()).await {
                    entity_tracker.read().await.is_visible_to(eid, me.x, me.z)
                } else {
                    false
                };

                if !visible {
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
            Ok((eid, entity_flags, skin_parts)) = meta_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                if eid == state.entity_id {
                    continue;
                }
                send_packet(&mut framed, EntityMetadata {
                    entity_id: eid,
                    entity_flags,
                    skin_parts
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
            Ok((eid, x, y, z, stage)) = break_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                if eid == state.entity_id {
                    continue;
                }
                send_packet(&mut framed, BlockBreakAnimation {
                    entity_id: eid,
                    x, y, z,
                    destroy_stage: stage
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
                    if let Some(me) = player_registry.get(&state.uuid.unwrap_or_default()).await {
                        let dx = player.x - me.x;
                        let dz = player.z - me.z;
                        let dist = (dx * dx + dz * dz).sqrt();
                        if dist > config.tracking.player {
                            continue;
                        }
                    }
                    send_packet(&mut framed, SpawnPlayer {
                        entity_id: player.entity_id,
                        uuid: player.uuid,
                        username: player.username.clone(),
                        properties: player.properties.clone(),
                        x: player.x,
                        y: player.y,
                        z: player.z,
                        yaw: 90.0,
                        pitch: 0.0,
                        current_item: 0
                    }).await;

                    send_packet(&mut framed, EntityMetadata {
                        entity_id: player.entity_id,
                        entity_flags: player.entity_flags(),
                        skin_parts: player.skin_parts
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

                send_packet(&mut framed, EntityStatus {
                    entity_id: state.entity_id,
                    status: 2,
                }).await;

                if health > 0.0
                    && let Some(uuid_val) = state.uuid
                    && let Some(me) = player_registry.get(&uuid_val).await
                {
                    let players = player_registry.get_all().await;
                    if let Some(attacker) = players.iter().find(|p| p.entity_id == attacker_eid) {
                        let dx = me.x - attacker.x;
                        let dz = me.z - attacker.z;
                        let magnitude = (dx * dx + dz * dz).sqrt().max(0.0001);

                        let horizontal = 0.4f64;
                        let mut vx = -(dx / magnitude) * horizontal;
                        let mut vy = 0.2f64;
                        let mut vz = -(dz / magnitude) * horizontal;

                        if attacker.is_sprinting {
                            let yaw_rad = attacker.yaw * std::f32::consts::PI / 180.0;
                            let sin_yaw = -yaw_rad.sin() as f64;
                            let cos_yaw = yaw_rad.cos() as f64;
                            let sprint_horizontal = 0.5f64;
                            let sprint_vertical = 0.1f64;

                            vx += sin_yaw * sprint_horizontal;
                            vy = sprint_vertical;
                            vz += cos_yaw * sprint_horizontal;
                        }
                        send_packet(&mut framed, EntityVelocity {
                            entity_id: state.entity_id,
                            vx,
                            vy,
                            vz
                        }).await;
                    }
                }
            }
            Ok((eid, x, y, z)) = item_rx.recv() => {
                if framed.codec().state != EnumProtocol::Play {
                    continue;
                }
                send_packet(&mut framed, SpawnObject {
                    entity_id: eid,
                    object_type: 2, // itemstack
                    x, y, z,
                    yaw: 0,
                    pitch: 0,
                    data: 1, // non zero to send velocity
                    vx: 0,
                    vy: 100,
                    vz: 0,
                }).await;
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

                        if framed.codec().state == EnumProtocol::Login {
                            // TODO: connection throttled
                            if !ALLOWED_PROTOCOLS.contains(&client_protocol) {
                                kick(&mut framed, "Unsupported version. Use 1.7.10 or 1.8.9").await;
                                break;
                            }
                            if player_registry.get_online_count().await > config.server.max_player {
                                kick(&mut framed, "Server is full!").await;
                                break;
                            }
                            if let Some(ip) = peer_ip
                                && let Some(ban) = banlist.read().await.is_ip_banned(&ip.ip())
                            {
                                kick(&mut framed, &format!("§cYou are IP banned from this server!\n§7Reason: §f{}", ban.reason)).await;
                                break;
                            }
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
                                    let uuid = Uuid::parse_str(&profile.uuid).unwrap_or_else(|_| Uuid::new_v4());
                                    if let Some(ban) = banlist.read().await.is_player_banned(&uuid) {
                                        kick(&mut framed, &format!("§cYou are banned!\n§7Reason: §f{}", ban.reason)).await;
                                        break;
                                    }
                                    else if config.server.whitelisted
                                        && !whitelist.read().await.is_whitelisted(uuid)
                                    {
                                        kick(&mut framed, "You're not whitelisted on this server!").await;
                                        break;
                                    }

                                    framed.codec_mut().encryption = Some(Encryption::new(&shared_secret));

                                    make_player_join(&mut framed, &mut state, uuid, profile, client_protocol, config.server.max_player as u8, &peer_ip, &player_registry, &join_tx, &chat_tx, &world_blocks, &entity_tracker,&config).await;
                                    continue;
                                }
                            } else {
                                if let Some(login_start) = packet.as_any().downcast_ref::<LoginStart>() {
                                    let uuid = Uuid::new_v3(
                                        &Uuid::NAMESPACE_DNS,
                                        format!("OfflinePlayer:{}", login_start.username).as_bytes(),
                                    );

                                    if let Some(ban) = banlist.read().await.is_player_banned(&uuid) {
                                        kick(&mut framed, &format!("§cYou are banned!\n§7Reason: §f{}", ban.reason)).await;
                                        break;
                                    }
                                    else if config.server.whitelisted
                                        && !whitelist.read().await.is_whitelisted(uuid)
                                    {
                                        kick(&mut framed, "You're not whitelisted on this server!").await;
                                        break;
                                    }

                                    let profile = AuthProfile {
                                        uuid: uuid.to_string(),
                                        username: login_start.username.clone(),
                                        properties: vec![]
                                    };

                                    make_player_join(&mut framed, &mut state, uuid, profile, client_protocol, config.server.max_player as u8, &peer_ip, &player_registry, &join_tx, &chat_tx, &world_blocks, &entity_tracker, &config).await;
                                    continue;
                                }
                            }
                            // TODO: check login from another location
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
                            let slot = held.slot.clamp(0, 8) as u8;
                            state.held_slot = slot;
                            if let Some(uuid) = state.uuid {
                                player_registry.update_held_slot(uuid, slot).await;
                            }
                            state.held_item = state.inventory.slots[slot as usize]
                                .as_ref()
                                .map(|s| s.item_id)
                                .unwrap_or(-1);
                            continue;
                        }

                        if let Some(dig) = packet.as_any().downcast_ref::<PlayerDig>() {
                            match dig.status {
                                0 if state.gamemode == 1 => {
                                    world_blocks.set(dig.x, dig.y, dig.z, Block::air()).await;
                                    block_tx.send((dig.x, dig.y as i32, dig.z, 0, 0)).ok();
                                }
                                0 if state.gamemode == 0 => {
                                    state.breaking_block = Some((dig.x, dig.y as i32, dig.z));
                                    break_tx.send((state.entity_id, dig.x, dig.y as i32, dig.z, 0)).ok();
                                }
                                1 => {
                                    if let Some((bx, by, bz)) = state.breaking_block.take() {
                                        break_tx.send((state.entity_id, bx, by, bz, 255)).ok();
                                    }
                                }
                                2 if state.gamemode == 0 => {
                                    if let Some((bx, by, bz)) = state.breaking_block.take() {
                                        let block = world_blocks.get(bx, by as u8, bz).await;
                                        world_blocks.set(bx, by as u8, bz, Block::air()).await;
                                        block_tx.send((
                                            bx,
                                            by,
                                            bz,
                                            0,
                                            0)
                                        ).ok();
                                        break_tx.send((state.entity_id, bx, by, bz, 10)).ok();

                                        if !block.is_air() && block.id > 0 {
                                            let drop_eid = next_entity_id();
                                            let x = bx as f64 + 0.5;
                                            let y = by as f64 + 0.5;
                                            let z = bz as f64 + 0.5;
                                            item_tx.send((
                                                drop_eid,
                                                x,
                                                y,
                                                z)
                                            ).ok();
                                            item_spawn_times.write().await.insert(drop_eid, Instant::now());
                                            entity_tracker.write().await.track(
                                                TrackedEntity::item(
                                                    drop_eid,
                                                    x, y, z,
                                                    config.tracking.item,
                                                )
                                            );
                                            item_positions.write().await.insert(
                                                drop_eid,
                                                (drop_eid, x, y, z, block.id as i16, 1, block.metadata as i16)
                                            );
                                        }
                                    }
                                }
                                3 | 4 => {
                                    let hotbar_slot = state.held_slot as usize;

                                    if state.inventory.slots[hotbar_slot].is_none() {
                                        continue;
                                    }

                                    let item = if dig.status == 3 {
                                        state.inventory.slots[hotbar_slot].take()
                                    } else {
                                        if let Some(slot) = state.inventory.slots[hotbar_slot].as_mut() {
                                            slot.count -= 1;
                                            let dropped = Slot {
                                                item_id: slot.item_id,
                                                count: 1,
                                                metadata: slot.metadata
                                            };
                                            if slot.count == 0 {
                                                state.inventory.slots[hotbar_slot] = None;
                                            }
                                            Some(dropped)
                                        } else {
                                            None
                                        }
                                    };

                                    if let Some(dropped) = item {
                                        let packet_slot = (36 + hotbar_slot) as i16;
                                        let remaining = state.inventory.slots[hotbar_slot].as_ref();

                                        send_packet(&mut framed, SetSlot {
                                            window_id: 0,
                                            slot: packet_slot,
                                            item_id: remaining.map(|s| s.item_id).unwrap_or(-1),
                                            count: remaining.map(|s| s.count).unwrap_or(0),
                                            metadata: remaining.map(|s| s.metadata).unwrap_or(0)
                                        }).await;

                                        state.held_item = state.inventory.slots[hotbar_slot]
                                            .as_ref()
                                            .map(|s| s.item_id)
                                            .unwrap_or(-1);

                                        if let Some(p) = player_registry.get(&state.uuid.unwrap()).await {
                                            let yaw_rad = p.yaw * std::f32::consts::PI / 180.0;
                                            let drop_x = p.x + (-yaw_rad.sin() * 0.5) as f64;
                                            let drop_y = p.y + 1.0;
                                            let drop_z = p.z + (yaw_rad.cos() * 0.5) as f64;

                                            let drop_eid = next_entity_id();
                                            item_tx.send((drop_eid, drop_x, drop_y, drop_z)).ok();
                                            item_spawn_times.write().await.insert(drop_eid, Instant::now());
                                            item_positions.write().await.insert(
                                                drop_eid,
                                                (drop_eid, drop_x, drop_y, drop_z,
                                                dropped.item_id, dropped.count, dropped.metadata)
                                            );
                                        }

                                        send_packet(&mut framed, HeldItemChange {
                                            slot: state.held_slot as i16
                                        }).await;
                                    }
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

                            if state.gamemode == 0 {
                                let hotbar_slot = state.held_slot as usize;
                                if let Some(slot) = state.inventory.slots[hotbar_slot].as_mut() {
                                    slot.count -= 1;
                                    let remaining_count = slot.count;
                                    let item_id = slot.item_id;
                                    let metadata = slot.metadata;

                                    if remaining_count == 0 {
                                        state.inventory.slots[hotbar_slot] = None;
                                    }

                                    let packed_slot = (36 + hotbar_slot) as i16;
                                    send_packet(&mut framed, SetSlot {
                                        window_id: 0,
                                        slot: packed_slot,
                                        item_id: if remaining_count > 0 { item_id } else { -1 },
                                        count: remaining_count,
                                        metadata
                                    }).await;

                                    state.held_item = state.inventory.slots[hotbar_slot]
                                        .as_ref()
                                        .map(|s| s.item_id)
                                        .unwrap_or(-1);
                                } else {
                                    continue;
                                }
                            }

                            state.held_item = place.held_item_id;
                            if let Some(uuid) = state.uuid {
                                player_registry.update_held_item(uuid, state.held_item).await;
                            }

                            world_blocks.set(tx, ty as u8, tz, Block::new(block_id as u8, 0)).await;
                            block_tx.send((tx, ty, tz, block_id, 0)).ok();
                            continue;
                        }

                        if packet.as_any().downcast_ref::<ArmAnimation>().is_some() {
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
                                        send_packet(&mut framed, ChatBuilder::new(&msg).into_packet()).await;
                                    }
                                    CommandResult::Error(msg) => {
                                        send_packet(&mut framed, ChatBuilder::new(&msg).color(ChatColor::Red).into_packet()).await;
                                    }
                                    CommandResult::Broadcast(msg) => {
                                        chat_tx.send(ChatBuilder::plain_json(&msg)).ok();
                                    }
                                    CommandResult::None => {}
                                }
                                continue;
                            }
                            if let Some(ref name) = state.name {
                                let max_len = match state.client_brand.as_deref() {
                                    Some(brand) if brand.contains("forge") || brand.contains("fabric") => 256,
                                    _ => 100,
                                };
                                if chat.message.len() > max_len {
                                    continue;
                                }
                                let json = ChatBuilder::chat_message(&config.chat.format, name, &chat.message);
                                //println!("[CHAT] {}", json);
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
                                entity_tracker.write().await.update_position(
                                    state.entity_id,
                                    pos.x, pos.y, pos.z
                                );
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
                                    let distance = ((pos.x - p.x).powi(2) + (pos.z - p.z).powi(2)).sqrt();

                                    if state.is_sprinting {
                                        state.food_exhaustion += 0.1 * distance as f32;
                                    } else {
                                        state.food_exhaustion += 0.01 * distance as f32;
                                    }
                                    player_registry.update_position(&uuid, pos.x, pos.y, pos.z, p.yaw, p.pitch, pos.on_ground).await;
                                    pos_tx.send((uuid, state.entity_id, pos.x, pos.y, pos.z, p.yaw, p.pitch, pos.on_ground)).ok();
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

                        if packet.as_any().downcast_ref::<CloseWindow>().is_some() {
                            // TODO
                            continue;
                        }

                        if packet.as_any().downcast_ref::<ConfirmTransaction>().is_some() {
                            // client acknowledged our confirmation
                            continue;
                        }

                        if let Some(creative) = packet.as_any().downcast_ref::<CreativeInventoryAction>() {
                            if state.gamemode != 1 {
                                continue;
                            }

                            let internal = Inventory::packet_to_internal(creative.slot);
                            if let Some(idx) = internal {
                                if creative.item_id == -1 {
                                    state.inventory.slots[idx] = None;
                                } else {
                                    state.inventory.slots[idx] = Some(Slot {
                                        item_id: creative.item_id,
                                        count: creative.item_count,
                                        metadata: creative.item_damage
                                    });
                                }
                                if idx < 9
                                    && idx == state.held_slot as usize
                                {
                                    state.held_item = creative.item_id;
                                    if let Some(uuid) = state.uuid {
                                        player_registry.update_held_item(uuid, state.held_item).await;
                                    }
                                }
                            }
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

                                send_chunks(&mut framed, client_protocol, &world_blocks).await;

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
                                    3 => {
                                        // server authority dont allow sprint if food < 6
                                        if state.food > 6 {
                                            Some((false, true))  // sprinting on
                                        } else {
                                            // deny sprint
                                            send_packet(&mut framed, PlayerAbilities {
                                                flags: 0x00,
                                                fly_speed: 0.05,
                                                walk_speed: 0.1,
                                            }).await;
                                            None
                                        }
                                    }
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
                                        meta_tx.send((state.entity_id, player.entity_flags(), player.skin_parts)).ok();
                                    }
                                }
                            }
                            continue;
                        }

                        if let Some(use_entity) = packet.as_any().downcast_ref::<UseEntity>() {
                            if use_entity.action == 1 && state.gamemode != 3 {
                                let players = player_registry.get_all().await;
                                if let Some(target) = players.iter()
                                    .find(|p| p.entity_id == use_entity.target_entity_id)
                                {
                                    if target.is_dead || target.no_damage_ticks > 0 {
                                        continue;
                                    }
                                    player_registry.update_no_damage_ticks(target.uuid, 10).await;
                                    if let Some(me) = player_registry.get(&state.uuid.unwrap()).await {
                                        let reach = if state.gamemode == 1 {
                                            5.0
                                        } else {
                                            3.0
                                        };
                                        let dx = me.x - target.x;
                                        let dy = me.y - target.y;
                                        let dz = me.z - target.z;
                                        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                                        if dist > reach {
                                            continue;
                                        }
                                        let new_health = (target.health - 1.0).max(2.0);
                                        player_registry.update_health(target.uuid, new_health, target.food, target.food_saturation).await;

                                        dmg_tx.send((target.uuid, new_health, target.food, target.food_saturation, state.entity_id)).ok();
                                        status_tx.send((use_entity.target_entity_id, 2)).ok();
                                        anim_tx.send((use_entity.target_entity_id, 1)).ok();
                                    }
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

                        if let Some(settings) = packet.as_any().downcast_ref::<ClientSettings>() {
                            state.skin_parts = settings.skin_parts;
                            if let Some(uuid) = state.uuid {
                                player_registry.update_skin_parts(uuid, settings.skin_parts).await;
                            }
                            meta_tx.send((state.entity_id, 0x00, settings.skin_parts)).ok();
                            continue;
                        }

                        if let Some(plugin) = packet.as_any().downcast_ref::<PluginMessage>() {
                            if plugin.channel == "MC|Brand" {
                                if let Ok(brand) = String::from_utf8(plugin.data.clone()) {
                                    let brand = brand.trim_matches('\0').to_string();
                                    state.client_brand = Some(brand);
                                }
                                send_packet(&mut framed, PluginMessage::brand("Coral")).await;
                            }
                            continue;
                        }

                        println!("WARN: Unhandled packet: {:?}", packet);
                        continue;
                    }
                    Err(e) => {
                        if !is_normal_disconnect(&e) {
                            eprintln!("Error processing packet: {:?}", e);
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
            .send((
                Player::new(state.entity_id, uuid, String::new(), vec![]),
                false,
            ))
            .ok();
        entity_tracker.write().await.untrack(state.entity_id);
        if !player_registry.players.read().await.is_empty() {
            chat_tx
                .send(ChatBuilder::colored_json(
                    &format!("{} left the game", name),
                    ChatColor::Yellow,
                ))
                .ok();
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

    player_registry
        .update_health(uuid, *health, *food, *food_saturation)
        .await;

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

async fn send_weather(framed: &mut Framed<TcpStream, Codec>, weather: WeatherState) {
    match weather {
        WeatherState::Clear => {
            send_packet(
                framed,
                ChangeGameState {
                    reason: 1,
                    value: 0.0,
                },
            )
            .await;
        }
        WeatherState::Rain => {
            send_packet(
                framed,
                ChangeGameState {
                    reason: 2,
                    value: 0.0,
                },
            )
            .await;
            send_packet(
                framed,
                ChangeGameState {
                    reason: 7,
                    value: 1.0,
                },
            )
            .await;
            send_packet(
                framed,
                ChangeGameState {
                    reason: 8,
                    value: 0.0,
                },
            )
            .await;
        }
        WeatherState::Thunder => {
            send_packet(
                framed,
                ChangeGameState {
                    reason: 2,
                    value: 0.0,
                },
            )
            .await;
            send_packet(
                framed,
                ChangeGameState {
                    reason: 7,
                    value: 1.0,
                },
            )
            .await;
            send_packet(
                framed,
                ChangeGameState {
                    reason: 8,
                    value: 1.0,
                },
            )
            .await;
        }
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

async fn send_chunks(
    framed: &mut Framed<TcpStream, Codec>,
    client_protocol: i32,
    world_blocks: &Arc<WorldBlocks>,
) {
    for cx in -2i32..=2 {
        for cz in -2i32..=2 {
            let chunk = ChunkData::build(cx, cz, client_protocol, world_blocks).await;
            send_packet(framed, chunk).await;
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn make_player_join(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    uuid: Uuid,
    profile: AuthProfile,
    client_protocol: i32,
    max_players: u8,
    peer_ip: &Option<SocketAddr>,
    player_registry: &Arc<PlayerRegistry>,
    join_tx: &Arc<broadcast::Sender<JoinLeave>>,
    chat_tx: &Arc<broadcast::Sender<String>>,
    world_blocks: &Arc<WorldBlocks>,
    entity_tracker: &Arc<RwLock<EntityTracker>>,
    config: &Config,
) {
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
                ping: state.latency_ms as i32,
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
                ping: state.latency_ms as i16,
            },
        )
        .await;
    }

    /*send_packet(
        framed,
        TimeUpdate {
            world_age: 0,
            time_of_day: 6000,
        },
    )
    .await;*/

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

    send_chunks(framed, client_protocol, world_blocks).await;

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

    let player = Player::new(
        entity_id,
        uuid,
        profile.username.clone(),
        profile.properties.clone(),
    );
    player_registry.add(player.clone()).await;

    chat_tx
        .send(ChatBuilder::colored_json(
            &format!("{} joined the game", profile.username),
            ChatColor::Yellow,
        ))
        .ok();

    println!(
        "[INFO] {}[/{}] logged in with entity id {}, at ([DEV] {}, {}, {})",
        profile.username,
        peer_ip
            .map(|a| a.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        entity_id,
        player.x,
        player.y,
        player.z
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
                properties: p.properties.clone(),
                x: p.x,
                y: p.y,
                z: p.z,
                yaw: 90.0,
                pitch: 0.0,
                current_item: 0,
            },
        )
        .await;
        send_packet(
            framed,
            EntityMetadata {
                entity_id: p.entity_id,
                entity_flags: p.entity_flags(),
                skin_parts: p.skin_parts,
            },
        )
        .await;
    }

    send_packet(
        framed,
        EntityMetadata {
            entity_id: player.entity_id,
            entity_flags: 0x00,
            skin_parts: state.skin_parts,
        },
    )
    .await;

    send_packet(
        framed,
        UpdateHealth {
            health: 20.0,
            food: 20,
            food_saturation: 5.0,
        },
    )
    .await;

    send_packet(
        framed,
        SetExperience {
            experience_bar: 0.0,
            level: 0,
            total_experience: 0,
        },
    )
    .await;

    join_tx.send((player, true)).ok();

    state.uuid = Some(uuid);
    state.entity_id = entity_id;
    state.name = Some(profile.username);
    state.gamemode = config.server.default_gamemode;
    entity_tracker.write().await.track(TrackedEntity::player(
        entity_id,
        uuid,
        0.5,
        4.5,
        0.5,
        config.tracking.player,
    ));
    let mut slots = Vec::with_capacity(46);
    for i in 0..46 {
        let internal = Inventory::packet_to_internal(i as i16);
        if let Some(idx) = internal
            && let Some(Some(s)) = state.inventory.slots.get(idx)
        {
            slots.push((s.item_id, s.count, s.metadata));
            continue;
        }
        slots.push((-1, 0, 0)); // empty
    }

    send_packet(
        framed,
        WindowItems {
            window_id: 0,
            slots,
        },
    )
    .await;
    send_weather(framed, state.current_weather.clone()).await;
}

fn is_normal_disconnect(e: &std::io::Error) -> bool {
    matches!(
        e.kind(),
        std::io::ErrorKind::TimedOut
            | std::io::ErrorKind::ConnectionReset
            | std::io::ErrorKind::ConnectionAborted
            | std::io::ErrorKind::BrokenPipe
            | std::io::ErrorKind::UnexpectedEof
    )
}
