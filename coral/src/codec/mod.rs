use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::vec;

use bytes::{Buf, Bytes, BytesMut};
use coral_server::effects::{ActiveEffect, EffectKind};
use coral_server::items::ItemRegistry;
use coral_server::items::armor::{apply_armor_reduction, total_defense};
use coral_server::items::drops::block_drop;
use coral_server::items::potions::PotionEffect;
use coral_server::mining::break_time_ticks;
use coral_server::projectile::{Projectile, ProjectileKind};
use coral_types::ToolMaterial;
use coral_world::generator::FlatWorldGenerator;
use coral_world::playerdata::{PlayerData, load_player_data, save_player_data};
use futures::SinkExt;
use tokio::net::TcpStream;
use tokio::sync::{RwLock, broadcast};
use tokio::time::interval;
use tokio_stream::StreamExt;
use tokio_util::codec::{Decoder, Encoder, Framed};
use uuid::Uuid;

use crate::{EquipmentUpdate, JoinLeave, ServerContext, SoundEffect};
use coral_command::{CommandContext, CommandResult};
use coral_config::Config;
use coral_protocol::auth::{AuthProfile, authenticate, compute_server_hash};
use coral_protocol::encryption::{Encryption, decrypt_rsa, generate_verify_token};
use coral_protocol::packets::handshake::keepalive::KeepAlive;
use coral_protocol::packets::login::disconnect::{LoginDisconnect, PlayDisconnect};
use coral_protocol::packets::login::{EncryptionRequest, EncryptionResponse, SetCompression};
use coral_protocol::packets::play::block::{
    BlockBreakAnimation, BlockChange, HeldItemChange, ItemEntityMetadata, PlayerBlockPlacement,
    PlayerDig,
};
use coral_protocol::packets::play::chat::builder::ChatBuilder;
use coral_protocol::packets::play::chat::builder::ChatColor;
use coral_protocol::packets::play::chat::{
    ChatMessage, ChatMessageOut, TabComplete, TabCompleteResponse,
};
use coral_protocol::packets::play::entity::{
    ArmAnimation, CollectItem, DestroyEntities, EntityAction, EntityAnimation, EntityEquipment,
    EntityHeadLook, EntityMetadata, EntityTeleport, EntityVelocity, SpawnObject, SpawnPlayer,
    UseEntity,
};
use coral_protocol::packets::play::game::{
    ChangeGameState, ClientStatus, EntityStatus, Respawn, SetExperience, UpdateHealth,
};
use coral_protocol::packets::play::inventory::{
    ClickWindow, CloseWindow, ConfirmTransaction, CreativeInventoryAction, Inventory, SetSlot,
    Slot, WindowItems,
};
use coral_protocol::packets::play::movement::{
    PlayerLook, PlayerMovements, PlayerOnGround, PlayerPosition, PlayerPositionAndLook,
};
use coral_protocol::packets::play::player_list::{
    PlayerListItem17, PlayerListItemAdd, PlayerListItemRemove, UpdateLatency,
};
use coral_protocol::packets::play::{
    ClientSettings, NamedSoundEffect, PluginMessage, WorldParticles,
};
use coral_protocol::packets::{PacketIn, PacketOut};
use coral_protocol::{
    packets::{
        PacketKey, PacketRegistry,
        handshake::{self, EnumProtocol, PacketHandshake},
        login::{LoginStart, LoginSuccess},
        play::{PlayerAbilities, SpawnPosition, SpawnPosition17, join_game::JoinGame},
        status::{Ping, Pong, Request, Response},
    },
    reader::Reader,
    writer::Writer,
};
use coral_server::{
    entity_tracker::{EntityTracker, TrackedEntity},
    player::Player,
    registry::{PlayerRegistry, next_entity_id},
};
use coral_world::{
    blocks::{Block, WorldBlocks},
    chunk::{ChunkData, UnloadChunk},
    time::TimeUpdate,
    weather::WeatherState,
};

mod ticking;

pub struct Codec {
    pub registry: Arc<PacketRegistry>,
    pub state: EnumProtocol,
    pub encryption: Option<Encryption>,
    compression_threshold: i32,
    decrypted_buf: BytesMut,
}

fn zlib_compress(data: &[u8]) -> Vec<u8> {
    use flate2::write::ZlibEncoder;
    use std::io::Write;
    let mut encoder = ZlibEncoder::new(Vec::new(), Default::default());
    encoder.write_all(data).unwrap();
    encoder.finish().unwrap()
}
fn zlib_decompress(data: &[u8]) -> std::io::Result<Vec<u8>> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;
    let mut decoder = ZlibDecoder::new(data);
    let mut out = Vec::new();
    decoder.read_to_end(&mut out)?;
    Ok(out)
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

        let inner: Bytes = if self.compression_threshold >= 0 {
            let mut r = Reader::new(&payload);
            let data_length = r.read_varint();
            let header_size = r.position;
            let rest = &payload[header_size..];

            if data_length == 0 {
                Bytes::from(rest.to_vec())
            } else {
                let decompressed = zlib_decompress(rest)?;
                Bytes::from(decompressed)
            }
        } else {
            Bytes::from(payload.to_vec())
        };

        let mut bytes = inner;

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

        let body: Vec<u8> = if self.compression_threshold >= 0 {
            if data.len() >= self.compression_threshold as usize {
                let compressed = zlib_compress(&data);
                let mut w = Writer::new();
                w.write_varint(data.len() as i32);
                w.data.extend_from_slice(&compressed);
                w.data
            } else {
                let mut w = Writer::new();
                w.write_varint(0);
                w.data.extend_from_slice(&data);
                w.data
            }
        } else {
            data
        };

        let mut length_writer = Writer::new();
        length_writer.write_varint(body.len() as i32);

        let mut frame = length_writer.data;
        frame.extend_from_slice(&body);

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
    absorption_hp: f32,
    active_effects: Vec<ActiveEffect>,
    is_dead: bool,
    is_sneaking: bool,
    is_sprinting: bool,
    is_flying: bool,
    was_on_ground: bool,
    latency_ms: u32,
    name: Option<String>,
    pending_username: Option<String>,
    keep_alive_count: i32,
    last_sent_keep_alive: Option<(i32, std::time::Instant)>,
    inventory: Inventory,
    breaking_block: Option<(i32, i32, i32)>,
    breaking_started_tick: i64,
    breaking_required_ticks: u32,
    current_weather: WeatherState,
    client_brand: Option<String>,
    skin_parts: u8,
    tick_count: i64,
    chunk_x: i32,
    chunk_z: i32,
    loaded_chunks: HashSet<(i32, i32)>,
    fall_distance: f32,
    eating: Option<Instant>,
    bow_charging: Option<Instant>,
    fishing_hook_eid: Option<i32>,
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
            absorption_hp: 0.0,
            active_effects: vec![],
            is_dead: false,
            is_sneaking: false,
            is_sprinting: false,
            is_flying: false,
            was_on_ground: true,
            latency_ms: 0,
            name: None,
            pending_username: None,
            keep_alive_count: 0,
            last_sent_keep_alive: None,
            inventory: Inventory::new(),
            breaking_block: None,
            breaking_started_tick: 0,
            breaking_required_ticks: 0,
            current_weather: WeatherState::Clear,
            client_brand: None,
            skin_parts: 0x7F,
            tick_count: 0,
            chunk_x: 0,
            chunk_z: 0,
            loaded_chunks: HashSet::new(),
            fall_distance: 0.0,
            eating: None,
            bow_charging: None,
            fishing_hook_eid: None,
        }
    }

    fn get_equipped_armor(&self) -> (i16, i16, i16, i16) {
        let helmet = self.inventory.slots[5]
            .as_ref()
            .map(|s| s.item_id)
            .unwrap_or(-1);
        let chest = self.inventory.slots[6]
            .as_ref()
            .map(|s| s.item_id)
            .unwrap_or(-1);
        let legs = self.inventory.slots[7]
            .as_ref()
            .map(|s| s.item_id)
            .unwrap_or(-1);
        let boots = self.inventory.slots[8]
            .as_ref()
            .map(|s| s.item_id)
            .unwrap_or(-1);
        (helmet, chest, legs, boots)
    }

    // TODO: when more projectile: match per kind & rename at consume_projectile_from_inventory
    fn consume_arrow_from_inventory(&mut self) {
        for slot in self.inventory.slots.iter_mut() {
            if let Some(s) = slot
                && s.item_id == 262
            {
                s.count -= 1;
                if s.count == 0 {
                    *slot = None;
                }
                return;
            }
        }
    }

    fn damage_item(&mut self, cost: i16, item_registry: &Arc<ItemRegistry>) -> bool {
        let slot_idx = self.held_slot as usize;
        let Some(slot) = self.inventory.slots[slot_idx].as_mut() else {
            return false;
        };

        let Some(max_dur) = item_registry.max_durability(slot.item_id) else {
            return false;
        };

        slot.metadata += cost;
        if slot.metadata >= max_dur {
            self.inventory.slots[slot_idx] = None;
            self.held_item = -1;
            return true;
        }
        false
    }

    async fn sync_armor(
        &self,
        player_registry: &Arc<PlayerRegistry>,
        equip_tx: &Arc<broadcast::Sender<EquipmentUpdate>>,
    ) {
        let (helmet, chest, legs, boots) = self.get_equipped_armor();

        if let Some(uuid) = self.uuid {
            player_registry
                .update_armor(uuid, helmet, chest, legs, boots)
                .await;
        }

        equip_tx.send((self.entity_id, 1, boots, 1, 0)).ok();
        equip_tx.send((self.entity_id, 2, legs, 1, 0)).ok();
        equip_tx.send((self.entity_id, 3, chest, 1, 0)).ok();
        equip_tx.send((self.entity_id, 4, helmet, 1, 0)).ok();
    }

    async fn try_retract_fishing_hook(
        &mut self,
        projectiles: &RwLock<Vec<Projectile>>,
        despawn_tx: &broadcast::Sender<i32>,
    ) -> bool {
        let Some(hook_eid) = self.fishing_hook_eid.take() else {
            return false;
        };
        projectiles
            .write()
            .await
            .retain(|p| p.entity_id != hook_eid);
        despawn_tx.send(hook_eid).ok();
        true
    }
}

pub async fn process(socket: TcpStream, ctx: ServerContext) {
    let ServerContext {
        packet_registry,
        player_registry,
        item_registry,
        block_registry,
        server_icon,
        config,
        dispatcher,
        entity_tracker,
        item_spawn_times,
        item_positions,
        projectiles,
        channels,
        world_blocks,
        generator,
        private_key,
        public_key_der,
        ops,
        whitelist,
        banlist,
        spawn_point,
        world_dir,
    } = ctx;
    let codec = Codec {
        registry: packet_registry,
        state: EnumProtocol::Handshaking,
        encryption: None,
        compression_threshold: -1,
        decrypted_buf: BytesMut::new(),
    };
    let peer_ip = socket.peer_addr().ok();
    let mut shutdown_rx = channels.shutdown_tx.subscribe();

    let mut state = PlayerState::new(config.server.default_gamemode);

    let mut framed = Framed::new(socket, codec);
    let mut client_protocol = -1;

    let verify_token = generate_verify_token();

    loop {
        tokio::select! {
                Ok(()) = shutdown_rx.recv() => {
                    if framed.codec().state == EnumProtocol::Login {
                        kick(&mut framed, "Server closed.").await;
                    }
                    return;
                }
                result = framed.next() => {
                    let Some(result) = result else {
                        return
                    };
                    match result {
                        Ok(packet) => {
                            match framed.codec().state {
                                EnumProtocol::Handshaking => {
                                    if let Some(handshake) = packet.as_any().downcast_ref::<PacketHandshake>() {
                                        client_protocol = handshake.protocol_version;
                                        framed.codec_mut().state = handshake.requested_protocol.clone();
                                    }
                                }
                                EnumProtocol::Status => {
                                    if packet.as_any().downcast_ref::<Request>().is_some() {
                                        let players = player_registry.get_all().await;
                                        let sample: Vec<(&str, String)> = players.iter()
                                            .take(config.server.player_sample_size as usize)
                                            .map(|p| (p.username.as_str(), p.uuid.hyphenated().to_string()))
                                            .collect();
                                        let sample_refs: Vec<(&str, &str)> = sample.iter()
                                            .map(|(name, uuid)| (*name, uuid.as_str()))
                                            .collect();

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
                                                config.server.max_players,
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
                                }
                                EnumProtocol::Login => {
                                    // TODO: connection throttled
                                    if !ALLOWED_PROTOCOLS.contains(&client_protocol) {
                                        kick(&mut framed, "Unsupported version. Use 1.8.x").await;
                                        return;
                                    }
                                    if player_registry.get_online_count().await > config.server.max_players {
                                        kick(&mut framed, "Server is full!").await;
                                        return;
                                    }
                                    if let Some(ip) = peer_ip
                                        && let Some(ban) = banlist.read().await.is_ip_banned(&ip.ip())
                                    {
                                        kick(&mut framed, &format!("§cYou are IP banned from this server!\n§7Reason: §f{}", ban.reason)).await;
                                        return;
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
                                                return;
                                            }
                                            let username = match state.pending_username.take() {
                                                Some(u) => u,
                                                None => return
                                            };

                                            let server_hash = compute_server_hash("", &shared_secret, &public_key_der);

                                            let profile = match authenticate(&username, &server_hash).await {
                                                Some(p) => p,
                                                None => {
                                                    kick(&mut framed, "Failed to verify username!").await;
                                                    return
                                                }
                                            };
                                            let uuid = Uuid::parse_str(&profile.uuid).unwrap_or_else(|_| Uuid::new_v4());
                                            if let Some(ban) = banlist.read().await.is_player_banned(&uuid) {
                                                kick(&mut framed, &format!("§cYou are banned!\n§7Reason: §f{}", ban.reason)).await;
                                                return;
                                            }
                                            else if config.server.whitelisted
                                                && !whitelist.read().await.is_whitelisted(uuid)
                                            {
                                                kick(&mut framed, "You're not whitelisted on this server!").await;
                                                return;
                                            }

                                            framed.codec_mut().encryption = Some(Encryption::new(&shared_secret));

                                            make_player_join(&mut framed,
                                                &mut state,
                                                uuid,
                                                profile,
                                                client_protocol,
                                                config.server.max_players as u8,
                                                &peer_ip,
                                                &player_registry,
                                                &channels.join_tx,
                                                &channels.chat_tx,
                                                &world_blocks,
                                                &generator,
                                                &entity_tracker,
                                                &config,
                                                &spawn_point,
                                                &world_dir
                                            ).await;
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
                                                return;
                                            }
                                            else if config.server.whitelisted
                                                && !whitelist.read().await.is_whitelisted(uuid)
                                            {
                                                kick(&mut framed, "You're not whitelisted on this server!").await;
                                                return;
                                            }

                                            let profile = AuthProfile {
                                                uuid: uuid.to_string(),
                                                username: login_start.username.clone(),
                                                properties: vec![]
                                            };

                                            make_player_join(&mut framed,
                                                &mut state,
                                                uuid,
                                                profile,
                                                client_protocol,
                                                config.server.max_players as u8,
                                                &peer_ip,
                                                &player_registry,
                                                &channels.join_tx,
                                                &channels.chat_tx,
                                                &world_blocks,
                                                &generator,
                                                &entity_tracker,
                                                &config,
                                                &spawn_point,
                                                &world_dir
                                            ).await;
                                            continue;
                                        }
                                    }
                                    // TODO: check login from another location
                                }
                                _ => {}
                            }
                        }
                        Err(e) => {
                            if !is_normal_disconnect(&e) {
                                eprintln!("Error processing packet: {:?}", e);
                            }
                            return;
                        }
                }
            }
        }
        if framed.codec().state == EnumProtocol::Play {
            break;
        }
    }
    let mut chat_rx = channels.chat_tx.subscribe();
    let mut join_rx = channels.join_tx.subscribe();
    let mut pos_rx = channels.pos_tx.subscribe();
    let mut gm_rx = channels.gm_tx.subscribe();
    let mut ping_rx = channels.ping_tx.subscribe();
    let mut block_rx = channels.block_tx.subscribe();
    let mut break_rx = channels.break_tx.subscribe();
    let mut anim_rx = channels.anim_tx.subscribe();
    let mut meta_rx = channels.meta_tx.subscribe();
    let mut dmg_rx = channels.dmg_tx.subscribe();
    let mut item_rx = channels.item_tx.subscribe();
    let mut despawn_rx = channels.despawn_tx.subscribe();
    let mut pickup_rx = channels.pickup_tx.subscribe();
    let mut time_rx = channels.time_tx.subscribe();
    let mut weather_rx = channels.weather_tx.subscribe();
    let mut tick_rx = channels.tick_tx.subscribe();
    let mut status_rx = channels.status_tx.subscribe();
    let mut equip_rx = channels.equip_tx.subscribe();
    let mut sound_rx = channels.sound_tx.subscribe();
    let mut particle_rx = channels.particle_tx.subscribe();
    let mut projectile_spawn_rx = channels.projectile_spawn_tx.subscribe();
    let mut projectile_move_rx = channels.projectile_move_tx.subscribe();
    let mut splash_effect_rx = channels.splash_effect_tx.subscribe();

    let mut keep_alive_interval = interval(Duration::from_secs(15)); // 30 seconds is timed out

    loop {
        tokio::select! {
            _ = keep_alive_interval.tick() => {
                state.keep_alive_count += 1;
                state.last_sent_keep_alive = Some((state.keep_alive_count, std::time::Instant::now()));
                send_packet(&mut framed, KeepAlive { id: state.keep_alive_count }).await;
            }
            Ok(()) = shutdown_rx.recv() => {
                kick(&mut framed, "Server closed.").await;
                break;
            }
            Ok(()) = tick_rx.recv() => {
                state.tick_count += 1;

                if state.is_dead {
                    continue;
                }
                ticking::handle_tick(&mut framed,
                    &mut state,
                    &player_registry,
                    &item_registry,
                    &config,
                    &item_spawn_times,
                    &item_positions,
                    &channels,
                ).await;
            }
            Ok((sender_eid, id, x, y, z, ox, oy, oz, data, count)) = particle_rx.recv() => {
                if state.entity_id == sender_eid {
                    continue;
                }
                // FIXME: cause crash
                /*send_packet(&mut framed, WorldParticles {
                    particle_id: id,
                    long_distance: false,
                    x, y, z,
                    offset_x: ox,
                    offset_y: oy,
                    offset_z: oz,
                    particle_data: data,
                    count
                }).await;*/
            }
            Ok((target_uuid, effect_id, amplifier, duration)) = splash_effect_rx.recv() => {
                if Some(target_uuid) != state.uuid {
                    continue;
                }

                let kind = match effect_id {
                    1  => EffectKind::Regeneration,
                    2  => EffectKind::Speed,
                    3  => EffectKind::FireResistance,
                    4  => EffectKind::Poison,
                    5  => EffectKind::InstantHealth,
                    6  => EffectKind::NightVision,
                    7  => EffectKind::InstantDamage,
                    8  => EffectKind::Weakness,
                    9  => EffectKind::Strength,
                    10 => EffectKind::Slowness,
                    11 => EffectKind::JumpBoost,
                    12 => EffectKind::InstantDamage,
                    13 => EffectKind::WaterBreathing,
                    14 => EffectKind::Invisibility,
                    _  => continue,
                };

                let pe = PotionEffect { kind, amplifier, duration_ticks: duration };
                apply_potion_effect(&mut framed, &mut state, &player_registry, pe).await;
            }
            Ok((eid, owner_eid, kind, x, y, z, vx, vy, vz)) = projectile_spawn_rx.recv() => {
                let object_type = kind.entity_id();

                send_packet(&mut framed, SpawnObject {
                    entity_id: eid,
                    object_type,
                    x, y, z,
                    yaw: 0, pitch: 0,
                    data: owner_eid + 1,
                    vx: (vx * 8000.0) as i16,
                    vy: (vy * 8000.0) as i16,
                    vz: (vz * 8000.0) as i16,
                }).await;
            }
            Ok((eid, x, y, z)) = projectile_move_rx.recv() => {
                send_packet(&mut framed, EntityTeleport {
                    entity_id: eid,
                    x, y, z,
                    yaw: 0, pitch: 0,
                    on_ground: false
                }).await;
            }
            Ok(weather) = weather_rx.recv() => {
                state.current_weather = weather.clone();
                send_weather(&mut framed, weather).await;
            }
            Ok(eid) = despawn_rx.recv() => {
                send_packet(&mut framed, DestroyEntities {
                    entity_ids: vec![eid]
                }).await;
            }
            Ok((eid, status)) = status_rx.recv() => {
                send_packet(&mut framed, EntityStatus {
                    entity_id: eid,
                    status,
                }).await;
            }
            Ok((sound, x, y, z, volume, pitch)) = sound_rx.recv() => {
                send_packet(&mut framed, NamedSoundEffect {
                    sound, x, y, z, volume, pitch
                }).await;
            }
            Ok((collector_eid, _collector_uuid, item_eid)) = pickup_rx.recv() => {
                send_packet(&mut framed, CollectItem {
                    collected_entity_id: item_eid,
                    collector_entity_id: collector_eid,
                }).await;
                send_packet(&mut framed, DestroyEntities {
                    entity_ids: vec![item_eid]
                }).await;
            }
            Ok((world_age, time_of_day)) = time_rx.recv() => {
                send_packet(&mut framed, TimeUpdate {
                    world_age,
                    time_of_day
                }).await;
            }
            Ok((eid, slot, item_id, count, metadata)) = equip_rx.recv() => {
                if eid == state.entity_id {
                    continue;
                }
                send_packet(&mut framed, EntityEquipment {
                    entity_id: eid,
                    slot,
                    item_id,
                    count,
                    metadata
                }).await;
            }

            Ok((uuid, eid, x, y, z, yaw, pitch, on_ground)) = pos_rx.recv() => {
                if Some(uuid) == state.uuid {
                    continue;
                }

                let was_on_ground = player_registry.get(&uuid).await
                    .map(|p| p.on_ground)
                    .unwrap_or(true);

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

                if on_ground && !was_on_ground {
                    let land_block = world_blocks.get(
                        x.floor() as i32,
                        (y.floor() as i32 - 1).max(0) as u8,
                        z.floor() as i32,
                        &generator
                    ).await;

                    if !land_block.is_air() {
                        channels.particle_tx.send((
                            state.entity_id,
                            37,
                            x as f32,
                            y as f32,
                            z as f32,
                            0.3, 0.0, 0.3,
                            land_block.id as f32,
                            6
                        )).ok();
                    }
                }

                let is_sprinting = player_registry.get(&uuid).await
                    .map(|p| p.is_sprinting)
                    .unwrap_or(false);

                if is_sprinting && on_ground {
                    let block_below = world_blocks.get(
                        x.floor() as i32,
                        (y.floor() as i32 - 1).max(0) as u8,
                        z.floor() as i32,
                        &generator
                    ).await;

                    if !block_below.is_air() {
                        let yaw_rad = (yaw * std::f32::consts::PI / 180.0) as f64;
                        let behind_x = x + yaw_rad.sin() * 0.2;
                        let behind_z = z - yaw_rad.cos() * 0.2;

                        channels.particle_tx.send((
                            state.entity_id,
                            37,
                            behind_x as f32,
                            y as f32,
                            behind_z as f32,
                            0.0, 0.0, 0.0,
                            block_below.id as f32,
                            3
                        )).ok();
                    }
                }
            }

            Ok((eid, anim)) = anim_rx.recv() => {
                if eid == state.entity_id {
                    continue;
                }
                send_packet(&mut framed, EntityAnimation {
                    entity_id: eid,
                    animation: anim
                }).await;
            }
            Ok((eid, entity_flags, skin_parts)) = meta_rx.recv() => {
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
                send_packet(&mut framed, BlockChange {
                    x, y, z,
                    block_id,
                    block_metadata: metadata
                }).await;
            }
            Ok((eid, x, y, z, stage)) = break_rx.recv() => {
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
                if Some(player.uuid) == state.uuid {
                    continue;
                }
                if !join_event {
                    send_packet(&mut framed, DestroyEntities {
                        entity_ids: vec![player.entity_id]
                    }).await;
                    send_packet(&mut framed, PlayerListItemRemove {
                        uuid: player.uuid
                    }).await;
                } else {
                    if client_protocol == 47 {
                        send_packet(&mut framed, PlayerListItemAdd {
                            uuid: player.uuid,
                            username: player.username.clone(),
                            properties: player.properties.clone(),
                            gamemode: player.gamemode as i32,
                            ping: player.latency_ms as i32
                        }).await;
                    } else {
                        send_packet(&mut framed, PlayerListItem17 {
                            username: player.username.clone(),
                            online: true,
                            ping: player.latency_ms as i16
                        }).await;
                    }
                    if let Some(me) = player_registry.get(&state.uuid.unwrap_or_default()).await {
                        let dx = player.x - me.x;
                        let dz = player.z - me.z;
                        let dist = (dx * dx + dz * dz).sqrt();
                        if dist > config.tracking.player {
                            continue;
                        }
                    }
                    send_spawn_player(&mut framed, &player).await;
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
                        let mut vx = (dx / magnitude) * horizontal;
                        let mut vy = 0.2f64;
                        let mut vz = (dz / magnitude) * horizontal;

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
            Ok((eid, x, y, z, item_id, count, metadata)) = item_rx.recv() => {
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
                send_packet(&mut framed, ItemEntityMetadata {
                    entity_id: eid,
                    item_id,
                    item_count: count,
                    item_damage: metadata
                }).await;
            }
            Ok(message) = chat_rx.recv() => {
                send_packet(&mut framed, ChatMessageOut::from_json(&message)).await;
            }
            Ok((uuid, ping)) = ping_rx.recv() => {
                send_packet(&mut framed, UpdateLatency {
                    uuid,
                    ping: ping as i32
                }).await;
            }
            Ok((_uuid, gamemode)) = gm_rx.recv() => {
                if state.uuid.is_some() {
                    state.gamemode = gamemode;
                    send_packet(&mut framed, ChangeGameState::set_gamemode(gamemode)).await;

                    let (flags, fly_speed, walk_speed) = match gamemode {
                        1 => (0x01 | 0x02 | 0x04 | 0x08, 0.05, 0.1),
                        3 => (0x01 | 0x02 | 0x04, 0.05, 0.1),
                        _ => (0x00, 0.05, 0.1),
                    };
                    send_packet(&mut framed, PlayerAbilities {
                        flags,
                        fly_speed,
                        walk_speed
                    }).await;
                }
            }
            result = framed.next() => {
                let Some(result) = result else {
                    break
                };
                match result {
                    Ok(packet) => {
                        if let Some(ka) = packet.as_any().downcast_ref::<KeepAlive>() {
                            if let Some((sent_id, sent_time)) = state.last_sent_keep_alive.take()
                                && ka.id == sent_id
                            {
                                state.latency_ms = sent_time.elapsed().as_millis() as u32;
                                if let Some(uuid) = state.uuid {
                                    player_registry.update_latency(uuid, state.latency_ms).await;
                                    channels.ping_tx.send((uuid, state.latency_ms)).ok();
                                }
                            }
                            continue;
                        }

                        if let Some(held) = packet.as_any().downcast_ref::<HeldItemChange>() {
                            let slot = held.slot.clamp(0, 8) as u8;
                            state.held_slot = slot;
                            state.held_item = state.inventory.slots[slot as usize]
                                .as_ref()
                                .map(|s| s.item_id)
                                .unwrap_or(-1);
                            if let Some(uuid) = state.uuid {
                                player_registry.update_held_slot(uuid, slot).await;
                                player_registry.update_held_item(uuid, state.held_item).await;
                            }
                            send_held_equip(&channels.equip_tx, &state);
                            state.eating = None;
                            state.bow_charging = None;
                            state.try_retract_fishing_hook(&projectiles, &channels.despawn_tx).await;
                            continue;
                        }

                        if let Some(dig) = packet.as_any().downcast_ref::<PlayerDig>() {
                            match dig.status {
                                0 if state.gamemode == 1 => {
                                    let block = world_blocks.get(dig.x, dig.y, dig.z, &generator).await;
                                    world_blocks.set(dig.x, dig.y, dig.z, Block::air(), &generator).await;
                                    channels.block_tx.send((dig.x, dig.y as i32, dig.z, 0, 0)).ok();
                                    channels.sound_tx.send((
                                        block_break_sound(block.id).to_string(),
                                        dig.x as f64 + 0.5, dig.y as f64 + 0.5, dig.z as f64 + 0.5,
                                        1.0, 63
                                    )).ok();
                                }
                                0 if state.gamemode == 0 => {
                                    let block = world_blocks.get(dig.x, dig.y, dig.z, &generator).await;
                                    let required = break_time_ticks(&item_registry, &block_registry, state.held_item, block.id, false, true);

                                    state.breaking_block = Some((dig.x, dig.y as i32, dig.z));
                                    state.breaking_started_tick = state.tick_count;
                                    state.breaking_required_ticks = required;
                                    channels.break_tx.send((state.entity_id, dig.x, dig.y as i32, dig.z, 0)).ok();
                                }
                                1 => {
                                    if let Some((bx, by, bz)) = state.breaking_block.take() {
                                        channels.break_tx.send((state.entity_id, bx, by, bz, 255)).ok();
                                    }
                                }
                                2 if state.gamemode == 0 => {
                                    if let Some((bx, by, bz)) = state.breaking_block.take() {
                                        let block = world_blocks.get(bx, by as u8, bz, &generator).await;

                                        let required_ticks = break_time_ticks(
                                            &item_registry,
                                            &block_registry,
                                            state.held_item,
                                            block.id,
                                            false,
                                            true,
                                        );
                                        let elapsed = (state.tick_count - state.breaking_started_tick).max(0) as u32;

                                        if elapsed < required_ticks {
                                            channels.block_tx.send((bx, by, bz, block.id as i32, block.metadata)).ok();
                                            continue;
                                        }
                                        world_blocks.set(bx, by as u8, bz, Block::air(), &generator).await;
                                        channels.block_tx.send((
                                            bx,
                                            by,
                                            bz,
                                            0,
                                            0
                                        )).ok();
                                        channels.break_tx.send((state.entity_id, bx, by, bz, 10)).ok();
                                        channels.particle_tx.send((state.entity_id, 37, bx as f32 + 0.5, by as f32 + 0.5, bz as f32 + 0.5, 0.3, 0.3, 0.3, block.id as f32, 8)).ok();
                                        channels.sound_tx.send((
                                            block_break_sound(block.id).to_string(),
                                            bx as f64 + 0.5, by as f64 + 0.5, bz as f64 + 0.5,
                                            1.0, 63
                                        )).ok();

                                        if !block.is_air() && block.id > 0 {
                                            let can_drop = if let Some(req_mat) = block_registry.required_material(block.id) {
                                                item_registry.get(state.held_item)
                                                    .and_then(|item| item.tool_material())
                                                    .map(|mat| material_meets(mat, req_mat))
                                                    .unwrap_or(false)
                                            } else {
                                                true
                                            };

                                            if can_drop
                                                && let Some((drop_id, drop_count, drop_metadata)) = block_drop(block.id, block.metadata)
                                            {
                                                let drop_eid = next_entity_id();
                                                let x = bx as f64 + 0.5;
                                                let y = by as f64 + 0.5;
                                                let z = bz as f64 + 0.5;
                                                channels.item_tx.send((
                                                    drop_eid,
                                                    x, y, z,
                                                    drop_id,
                                                    drop_count,
                                                    drop_metadata
                                                )).ok();
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
                                                    (drop_eid, x, y, z, drop_id, drop_count, drop_metadata)
                                                );
                                            }
                                        }
                                    }
                                    let broke = state.damage_item(1, &item_registry);
                                    sync_held_slot(&mut framed, &mut state, &player_registry, &channels.equip_tx, broke).await;
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
                                                metadata: slot.metadata,
                                                durability: 0
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
                                        if let Some(uuid) = state.uuid {
                                            player_registry.update_held_item(uuid, state.held_item).await;
                                        }
                                        send_held_equip(&channels.equip_tx, &state);

                                        if let Some(p) = player_registry.get(&state.uuid.unwrap_or_default()).await {
                                            let yaw_rad = p.yaw * std::f32::consts::PI / 180.0;
                                            let drop_x = p.x + (-yaw_rad.sin() * 0.5) as f64;
                                            let drop_y = p.y + 1.0;
                                            let drop_z = p.z + (yaw_rad.cos() * 0.5) as f64;

                                            let drop_eid = next_entity_id();
                                            channels.item_tx.send((
                                                drop_eid,
                                                drop_x, drop_y, drop_z,
                                                dropped.item_id,
                                                dropped.count,
                                                dropped.metadata
                                            )).ok();
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
                                5 => {
                                    if let Some(charge_start) = state.bow_charging.take()
                                        && state.held_item == 261 // TODO: In the future identify magic numbers!
                                    {
                                        let charge_secs = charge_start.elapsed().as_secs_f32().min(1.0);

                                        if charge_secs < 0.1 {
                                            continue;
                                        }

                                        let power = ((charge_secs * charge_secs + charge_secs * 2.0) / 3.0).clamp(0.0, 1.0);

                                        if let Some(uuid) = state.uuid
                                            && let Some(p) = player_registry.get(&uuid).await
                                        {
                                            let yaw_rad = p.yaw * std::f32::consts::PI / 180.0;
                                            let pitch_rad = p.pitch * std::f32::consts::PI / 180.0;

                                            let dx = (-yaw_rad.sin() * pitch_rad.cos()) as f64;
                                            let dy = (-pitch_rad.sin()) as f64;
                                            let dz = (yaw_rad.cos() * pitch_rad.cos()) as f64;

                                            let speed = power as f64 * 3.0;
                                            let arrow_eid = next_entity_id();

                                            let proj = Projectile {
                                                entity_id: arrow_eid,
                                                owner_entity_id: state.entity_id,
                                                kind: ProjectileKind::Arrow,
                                                x: p.x, y: p.y, z: p.z,
                                                vx: dx * speed,
                                                vy: dy * speed,
                                                vz: dz * speed,
                                                ticks_alive: 0
                                            };

                                            projectiles.write().await.push(proj.clone());
                                            channels.projectile_spawn_tx.send((
                                                arrow_eid, state.entity_id, ProjectileKind::Arrow,
                                                proj.x, proj.y, proj.z, proj.vx, proj.vy, proj.vz
                                            )).ok();

                                            channels.sound_tx.send(("random.bow".to_string(), p.x, p.y, p.z, 1.0, 63)).ok();

                                            if state.gamemode == 0 {
                                                state.consume_arrow_from_inventory();
                                            }
                                        }
                                        continue;
                                    }
                                    if state.held_item == 373 {
                                        let meta = state.inventory.slots[state.held_slot as usize].as_ref().map(|s| s.metadata).unwrap_or(0);
                                        let is_splash = (meta & 0x4000i16) != 0;
                                        if is_splash
                                            && let Some(uuid) = state.uuid
                                            && let Some(p) = player_registry.get(&uuid).await
                                        {
                                            let yaw_rad = p.yaw * std::f32::consts::PI / 180.0;
                                            let pitch_rad = p.pitch * std::f32::consts::PI / 180.0;

                                            let dx = (-yaw_rad.sin() * pitch_rad.cos()) as f64;
                                            let dy = (-pitch_rad.sin()) as f64;
                                            let dz = (yaw_rad.cos() * pitch_rad.cos()) as f64;

                                            let speed = 0.5;
                                            let proj_eid = next_entity_id();

                                            let proj = Projectile {
                                                entity_id: proj_eid,
                                                owner_entity_id: state.entity_id,
                                                kind: ProjectileKind::SplashPotion(meta),
                                                x: p.x, y: p.y + 1.5, z: p.z,
                                                vx: dx * speed,
                                                vy: dy * speed + 0.1,
                                                vz: dz * speed,
                                                ticks_alive: 0
                                            };

                                            projectiles.write().await.push(proj.clone());
                                            channels.projectile_spawn_tx.send((
                                                proj_eid, state.entity_id,
                                                ProjectileKind::SplashPotion(meta),
                                                proj.x, proj.y, proj.z,
                                                proj.vx, proj.vy, proj.vz
                                            )).ok();

                                            let hotbar = state.held_slot as usize;
                                            state.inventory.slots[hotbar] = None;
                                            send_packet(&mut framed, SetSlot {
                                                window_id: 0, slot: (36 + hotbar) as i16,
                                                item_id: -1, count: 0, metadata: 0
                                            }).await;
                                            state.held_item = -1;
                                            if let Some(uuid) = state.uuid {
                                                player_registry.update_held_item(uuid, -1).await;
                                            }
                                            send_held_equip(&channels.equip_tx, &state);
                                        }
                                    }
                                }
                                _ => {}
                            }
                            continue;
                        }

                        if let Some(place) = packet.as_any().downcast_ref::<PlayerBlockPlacement>() {
                            if place.face == 255 {
                                if let Some((_hunger, _saturation)) = item_registry.food_value(state.held_item)
                                    && state.food < 20
                                    && state.eating.is_none()
                                {
                                    state.eating = Some(Instant::now());
                                    channels.anim_tx.send((state.entity_id, 3)).ok();
                                }

                                match state.held_item {
                                    261 => {
                                        state.bow_charging = Some(Instant::now());
                                        continue;
                                    }
                                    346 => {
                                        if !state.try_retract_fishing_hook(&projectiles, &channels.despawn_tx).await
                                            && let Some(uuid) = state.uuid
                                            && let Some(p) = player_registry.get(&uuid).await
                                        {
                                            let yaw_rad = p.yaw * std::f32::consts::PI / 180.0;
                                            let pitch_rad = p.pitch * std::f32::consts::PI / 180.0;

                                            let dx = (-yaw_rad.sin() * pitch_rad.cos()) as f64;
                                            let dy = (-pitch_rad.sin()) as f64;
                                            let dz = (yaw_rad.cos() * pitch_rad.cos()) as f64;

                                            let speed = 1.5;
                                            let hook_eid = next_entity_id();

                                            let proj = Projectile {
                                                entity_id: hook_eid,
                                                owner_entity_id: state.entity_id,
                                                kind: ProjectileKind::FishingHook,
                                                x: p.x, y: p.y + 1.2, z: p.z, // TODO: change 1.2 to head_location
                                                vx: dx * speed,
                                                vy: dy * speed + 0.2,
                                                vz: dz * speed,
                                                ticks_alive: 0,
                                            };
                                            projectiles.write().await.push(proj.clone());
                                            channels.projectile_spawn_tx.send((
                                                hook_eid, state.entity_id, ProjectileKind::FishingHook,
                                                proj.x, proj.y, proj.z, proj.vx, proj.vy, proj.vz
                                            )).ok();

                                            state.fishing_hook_eid = Some(hook_eid);
                                        }
                                        channels.anim_tx.send((state.entity_id, 0)).ok();
                                        continue;
                                    }
                                    373 => {
                                        let meta = state.inventory.slots[state.held_slot as usize].as_ref().map(|s| s.metadata).unwrap_or(0);
                                        let is_splash = (meta & 0x4000) != 0;

                                        if !is_splash {
                                            if state.eating.is_none() {
                                                state.eating = Some(Instant::now());
                                                channels.anim_tx.send((state.entity_id, 3)).ok();
                                            }
                                            continue;
                                        }
                                    }
                                    _ => {}
                                }
                                continue;
                            }
                            if place.held_item_id == -1 {
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
                                    if let Some(uuid) = state.uuid {
                                        player_registry.update_held_item(uuid, state.held_item).await;
                                    }
                                    send_held_equip(&channels.equip_tx, &state);
                                } else {
                                    continue;
                                }
                            }

                            world_blocks.set(tx, ty as u8, tz, Block::new(block_id as u8, 0), &generator).await;
                            channels.block_tx.send((tx, ty, tz, block_id, 0)).ok();
                            channels.sound_tx.send((
                                block_break_sound(block_id as u8).to_string(),
                                tx as f64 + 0.5, ty as f64 + 0.5, tz as f64 + 0.5,
                                1.0, 63
                            )).ok();
                            continue;
                        }

                        if packet.as_any().downcast_ref::<ArmAnimation>().is_some() {
                            channels.anim_tx.send((state.entity_id, 0)).ok();
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
                                        send_packet(&mut framed, ChatMessageOut::from_json(&msg)).await;
                                    }
                                    CommandResult::Error(msg) => {
                                        send_packet(&mut framed, ChatBuilder::new(&msg).color(ChatColor::Red).into_packet()).await;
                                    }
                                    CommandResult::Broadcast(msg) => {
                                        channels.chat_tx.send(ChatBuilder::plain_json(&msg)).ok();
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
                                channels.chat_tx.send(json).ok();
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

                        let movement: Option<PlayerMovements> = None
                            .or_else(|| packet.as_any().downcast_ref::<PlayerPosition>().map(Into::into))
                            .or_else(|| packet.as_any().downcast_ref::<PlayerLook>().map(Into::into))
                            .or_else(|| packet.as_any().downcast_ref::<PlayerPositionAndLook>().map(Into::into))
                            .or_else(|| packet.as_any().downcast_ref::<PlayerOnGround>().map(Into::into));

                        if let Some(mv) = movement
                            && let Some(uuid) = state.uuid
                            && let Some(p) = player_registry.get(&uuid).await
                        {
                            let (x, y, z) = mv.position.unwrap_or((p.x, p.y, p.z));
                            let (yaw, pitch) = mv.rotation.unwrap_or((p.yaw, p.pitch));

                            if mv.position.is_some() {
                                if !mv.on_ground && y < p.y {
                                    state.fall_distance += (p.y - y) as f32;
                                } else if y > p.y {
                                    state.fall_distance = 0.0;
                                }

                                let moved = (x - p.x).abs() > 0.01
                                    || (y - p.y).abs() > 0.01
                                    || (z - p.z).abs() > 0.01;

                                if moved {
                                    entity_tracker.write().await.update_position(
                                        state.entity_id,
                                        x, y, z
                                    );
                                    let new_chunk_x = (x as i32) >> 4;
                                    let new_chunk_z = (z as i32) >> 4;

                                    if new_chunk_x != state.chunk_x || new_chunk_z != state.chunk_z {
                                        state.chunk_x = new_chunk_x;
                                        state.chunk_z = new_chunk_z;
                                        update_chunks(&mut framed, client_protocol, &world_blocks, &generator, new_chunk_x, new_chunk_z, config.server.view_distance, &mut state.loaded_chunks).await;
                                    }
                                    let distance = ((x - p.x).powi(2) + (z - p.z).powi(2)).sqrt();

                                    if state.is_sprinting {
                                        state.food_exhaustion += 0.1 * distance as f32;
                                    } else {
                                        state.food_exhaustion += 0.01 * distance as f32;
                                    }
                                }
                            }
                            handle_landing(&mut framed, &mut state, &player_registry, &channels.chat_tx, &channels.sound_tx, uuid, x, y, z, mv.on_ground).await;
                            player_registry.update_position(&uuid, x, y, z, yaw, pitch, mv.on_ground).await;
                            channels.pos_tx.send((uuid, state.entity_id, x, y, z, yaw, pitch, mv.on_ground)).ok();
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
                                        metadata: creative.item_damage,
                                        durability: 0,
                                    });
                                }
                                if idx < 9
                                    && idx == state.held_slot as usize
                                {
                                    state.held_item = creative.item_id;
                                    if let Some(uuid) = state.uuid {
                                        player_registry.update_held_item(uuid, state.held_item).await;
                                    }
                                    send_held_equip(&channels.equip_tx, &state);
                                }

                                if (5..=8).contains(&idx) {
                                    state.sync_armor(&player_registry, &channels.equip_tx).await;
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

                            if let Some(idx) = Inventory::packet_to_internal(click.slot)
                                && (5..=8).contains(&idx)
                            {
                                state.sync_armor(&player_registry, &channels.equip_tx).await;
                            }
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

                                state.loaded_chunks.clear();
                                send_chunks(
                                    &mut framed,
                                    client_protocol,
                                    &world_blocks,
                                    &generator,
                                    0, 0,
                                    config.server.view_distance,
                                    &mut state.loaded_chunks
                                ).await;

                                let (sx, sy, sz) = *spawn_point.read().await;

                                send_packet(&mut framed, PlayerPositionAndLook {
                                    x: sx,
                                    y: sy,
                                    z: sz,
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
                                        channels.meta_tx.send((state.entity_id, player.entity_flags(), player.skin_parts)).ok();
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
                                    if let Some(me) = player_registry.get(&state.uuid.unwrap_or_default()).await {
                                        let reach = if state.gamemode == 1 {
                                            5.0
                                        } else {
                                            4.0
                                        };
                                        let dx = me.x - target.x;
                                        let dy = me.y - target.y;
                                        let dz = me.z - target.z;
                                        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                                        if dist > reach {
                                            continue;
                                        }
                                        let strength_bonus = state.active_effects.iter()
                                            .find(|e| e.kind == EffectKind::Strength)
                                            .map(|e| 1.3 * (e.amplifier + 1) as f32)
                                            .unwrap_or(0.0);
                                        let weakness_penalty = state.active_effects.iter()
                                            .find(|e| e.kind == EffectKind::Weakness)
                                            .map(|e| 0.5 * (e.amplifier + 1) as f32)
                                            .unwrap_or(0.0);

                                        let base_damage = item_registry.attack_damage(state.held_item)
                                            + strength_bonus - weakness_penalty;
                                        let is_critical = !me.on_ground && state.fall_distance > 0.0;
                                        let raw_damage = (base_damage.max(0.0)) * if is_critical { 1.5 } else { 1.0 };

                                        let target_inventory_armor = player_registry.get_armor(&target.uuid).await;
                                        let total_armor = total_defense(
                                            &item_registry,
                                            target_inventory_armor.0,
                                            target_inventory_armor.1,
                                            target_inventory_armor.2,
                                            target_inventory_armor.3,
                                        );
                                        let resistance = player_registry.get_effects(&target.uuid).await.iter()
                                            .find(|e| e.kind == EffectKind::Resistance)
                                            .map(|e| 0.2 * (e.amplifier + 1) as f32)
                                            .unwrap_or(0.0);

                                        let damage = apply_armor_reduction(raw_damage, total_armor) * (1.0 - resistance);
                                        let new_health = (target.health - damage).max(0.0);

                                        player_registry.update_health(target.uuid, new_health, target.food, target.food_saturation).await;

                                        if new_health <= 0.0 {
                                            channels.chat_tx.send(ChatBuilder::plain_json(&format!(
                                                "{} was slain by {}",
                                                target.username,
                                                state.name.clone().unwrap_or_default()
                                            ))).ok();
                                        }

                                        channels.dmg_tx.send((target.uuid, new_health, target.food, target.food_saturation, state.entity_id)).ok();

                                        let broke = state.damage_item(2, &item_registry);
                                        sync_held_slot(&mut framed, &mut state, &player_registry, &channels.equip_tx, broke).await;

                                        channels.sound_tx.send((
                                            "game.player.hurt".to_string(),
                                            target.x, target.y, target.z,
                                            1.0, 63
                                        )).ok();
                                        channels.status_tx.send((use_entity.target_entity_id, 2)).ok();
                                        channels.anim_tx.send((use_entity.target_entity_id, 1)).ok();

                                        if is_critical {
                                            channels.anim_tx.send((state.entity_id, 4)).ok();
                                            channels.particle_tx.send((state.entity_id, 1, target.x as f32, target.y as f32 + 1.0, target.z as f32, 0.3, 0.3, 0.3, 0.0, 8)).ok();
                                        }
                                    }
                                }
                            }
                            continue;
                        }

                        if let Some(abilities) = packet.as_any().downcast_ref::<PlayerAbilities>() {
                            let is_flying = (abilities.flags & 0x02) != 0;
                            state.is_flying = is_flying;
                            continue;
                        }

                        if let Some(settings) = packet.as_any().downcast_ref::<ClientSettings>() {
                            state.skin_parts = settings.skin_parts;
                            if let Some(uuid) = state.uuid {
                                player_registry.update_skin_parts(uuid, settings.skin_parts).await;
                            }
                            channels.meta_tx.send((state.entity_id, 0x00, settings.skin_parts)).ok();
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
        let mut inventory_data = vec![];
        for i in 0..46i16 {
            if let Some(idx) = Inventory::packet_to_internal(i)
                && let Some(slot) = &state.inventory.slots[idx]
            {
                inventory_data.push((i, slot.item_id, slot.count, slot.metadata));
            }
        }
        if let Some(p) = player_registry.get(&uuid).await {
            let data = PlayerData {
                x: p.x,
                y: p.y,
                z: p.z,
                yaw: p.yaw,
                pitch: p.pitch,
                health: state.health,
                food: state.food,
                food_saturation: state.food_saturation,
                gamemode: state.gamemode,
                inventory: inventory_data,
            };
            save_player_data(&world_dir, &uuid, &data).await;
            channels.join_tx.send((p, false)).ok();
        }
        player_registry.remove(&uuid).await;
        entity_tracker.write().await.untrack(state.entity_id);
        if !player_registry.players.read().await.is_empty() {
            channels
                .chat_tx
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

fn send_held_equip(equip_tx: &Arc<broadcast::Sender<EquipmentUpdate>>, state: &PlayerState) {
    let (item_id, count, metadata) = state.inventory.slots[state.held_slot as usize]
        .as_ref()
        .map(|s| (s.item_id, s.count, s.metadata))
        .unwrap_or((-1, 0, 0));

    equip_tx
        .send((state.entity_id, 0, item_id, count, metadata))
        .ok();
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
) -> bool {
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
        return true;
    }
    false
}

async fn apply_potion_effect(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    pe: PotionEffect,
) {
    use EffectKind::*;
    match pe.kind {
        InstantHealth => {
            let heal = 4.0 * (1 << pe.amplifier) as f32;
            state.health = (state.health + heal).min(20.0 + state.absorption_hp);
            if let Some(uuid) = state.uuid {
                player_registry
                    .update_health(uuid, state.health, state.food, state.food_saturation)
                    .await;
            }
            send_packet(
                framed,
                UpdateHealth {
                    health: state.health,
                    food: state.food,
                    food_saturation: state.food_saturation,
                },
            )
            .await;
        }
        InstantDamage => {
            let dmg = 6.0 * (1 << pe.amplifier) as f32;
            damage_player(
                framed,
                &mut state.health,
                &mut state.food,
                &mut state.food_saturation,
                &mut state.is_dead,
                dmg,
                player_registry,
                state.uuid.unwrap_or_default(),
            )
            .await;
        }
        Absorption => {
            state.absorption_hp = 4.0 * (1 + pe.amplifier) as f32;
            apply_effect(
                framed,
                state,
                player_registry,
                ActiveEffect::new(pe.kind, pe.amplifier, pe.duration_ticks),
            )
            .await;
        }
        _ => {
            apply_effect(
                framed,
                state,
                player_registry,
                ActiveEffect::new(pe.kind, pe.amplifier, pe.duration_ticks),
            )
            .await;
        }
    }
}
async fn apply_effect(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    effect: ActiveEffect,
) {
    use coral_protocol::packets::play::game::EntityEffect;

    if let Some(existing) = state
        .active_effects
        .iter_mut()
        .find(|e| e.kind == effect.kind)
    {
        if effect.amplifier >= existing.amplifier {
            *existing = effect.clone();
        }
    } else {
        state.active_effects.push(effect.clone());
    }

    send_packet(
        framed,
        EntityEffect {
            entity_id: state.entity_id,
            effect_id: effect.kind as u8,
            amplifier: effect.amplifier,
            duration: effect.remaining_ticks,
            hide_particles: false,
        },
    )
    .await;

    if let Some(uuid) = state.uuid {
        player_registry
            .update_effects(uuid, state.active_effects.clone())
            .await;
    }
}
async fn remove_effect(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    kind: EffectKind,
) {
    use coral_protocol::packets::play::game::RemoveEntityEffect;

    state.active_effects.retain(|e| e.kind != kind);
    send_packet(
        framed,
        RemoveEntityEffect {
            entity_id: state.entity_id,
            effect_id: kind as u8,
        },
    )
    .await;
}

fn material_meets(have: ToolMaterial, need: ToolMaterial) -> bool {
    let rank = |m: ToolMaterial| match m {
        ToolMaterial::Wood | ToolMaterial::Gold => 0,
        ToolMaterial::Stone => 1,
        ToolMaterial::Iron => 2,
        ToolMaterial::Diamond => 3,
        ToolMaterial::Any => 0,
    };
    rank(have) >= rank(need)
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

async fn sync_held_slot(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    equip_tx: &Arc<broadcast::Sender<EquipmentUpdate>>,
    broke: bool,
) {
    let slot_idx = state.held_slot as usize;
    let packet_slot = (36 + slot_idx) as i16;
    let remaining = state.inventory.slots[slot_idx].as_ref();
    send_packet(
        framed,
        SetSlot {
            window_id: 0,
            slot: packet_slot,
            item_id: remaining.map(|s| s.item_id).unwrap_or(-1),
            count: remaining.map(|s| s.count).unwrap_or(0),
            metadata: remaining.map(|s| s.metadata).unwrap_or(0),
        },
    )
    .await;
    if broke {
        if let Some(uuid) = state.uuid {
            player_registry.update_held_item(uuid, -1).await;
        }
        send_held_equip(equip_tx, state);
    }
}

#[allow(clippy::too_many_arguments)]
async fn send_chunks(
    framed: &mut Framed<TcpStream, Codec>,
    client_protocol: i32,
    world_blocks: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
    center_x: i32,
    center_z: i32,
    view_distance: i32,
    loaded_chunks: &mut HashSet<(i32, i32)>,
) {
    for cx in (center_x - view_distance)..=(center_x + view_distance) {
        for cz in (center_z - view_distance)..=(center_z + view_distance) {
            if loaded_chunks.contains(&(cx, cz)) {
                continue;
            }
            let chunk = ChunkData::build(cx, cz, client_protocol, world_blocks, generator).await;
            send_packet(framed, chunk).await;
            loaded_chunks.insert((cx, cz));
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_landing(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    chat_tx: &Arc<broadcast::Sender<String>>,
    sound_tx: &Arc<broadcast::Sender<SoundEffect>>,
    uuid: Uuid,
    x: f64,
    y: f64,
    z: f64,
    on_ground: bool,
) {
    if on_ground && !state.was_on_ground {
        let damage_eligible = state.gamemode == 0 && !state.is_flying;

        if damage_eligible && state.fall_distance > 3.0 {
            let damage = (state.fall_distance - 3.0).round();
            let died = damage_player(
                framed,
                &mut state.health,
                &mut state.food,
                &mut state.food_saturation,
                &mut state.is_dead,
                damage,
                player_registry,
                uuid,
            )
            .await;
            let sound = if state.fall_distance > 7.0 {
                "game.player.hurt.fall.big"
            } else {
                "game.player.hurt.fall.small"
            };
            sound_tx.send((sound.to_string(), x, y, z, 1.0, 63)).ok();
            if died && let Some(ref name) = state.name {
                chat_tx
                    .send(ChatBuilder::plain_json(&format!(
                        "{} hit the ground too hard",
                        name
                    )))
                    .ok();
            }
        }
        state.fall_distance = 0.0;
    }
    state.was_on_ground = on_ground;
}

#[allow(clippy::too_many_arguments)]
async fn update_chunks(
    framed: &mut Framed<TcpStream, Codec>,
    client_protocol: i32,
    world_blocks: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
    new_chunk_x: i32,
    new_chunk_z: i32,
    view_distance: i32,
    loaded_chunks: &mut std::collections::HashSet<(i32, i32)>,
) {
    send_chunks(
        framed,
        client_protocol,
        world_blocks,
        generator,
        new_chunk_x,
        new_chunk_z,
        view_distance,
        loaded_chunks,
    )
    .await;

    let to_unload: Vec<(i32, i32)> = loaded_chunks
        .iter()
        .filter(|(cx, cz)| {
            (cx - new_chunk_x).abs() > view_distance + 1
                || (cz - new_chunk_z).abs() > view_distance + 1
        })
        .copied()
        .collect();

    for (cx, cz) in to_unload {
        loaded_chunks.remove(&(cx, cz));
        send_packet(
            framed,
            UnloadChunk {
                chunk_x: cx,
                chunk_z: cz,
            },
        )
        .await;
    }
}

fn block_break_sound(block_id: u8) -> &'static str {
    match block_id {
        2 | 3 | 60 => "dig.grass",
        1 | 4 | 7 | 14..=16 | 24 => "dig.stone",
        5 | 17 | 47 | 53 | 54 => "dig.wood",
        12 => "dig.sand",
        13 => "dig.gravel",
        20 | 102 => "dig.glass",
        35 => "dig.cloth",
        78 | 80 => "dig.snow",
        _ => "dig.stone",
    }
}

async fn send_player_equipment(
    framed: &mut Framed<TcpStream, Codec>,
    entity_id: i32,
    held: i16,
    helmet: i16,
    chestplate: i16,
    leggings: i16,
    boots: i16,
) {
    let slots = [
        (0, held),
        (4, helmet),
        (3, chestplate),
        (2, leggings),
        (1, boots),
    ];
    for (slot, item_id) in slots {
        if item_id == -1 {
            continue;
        }
        send_packet(
            framed,
            EntityEquipment {
                entity_id,
                slot,
                item_id,
                count: 1,
                metadata: 0,
            },
        )
        .await;
    }
}

async fn send_spawn_player(framed: &mut Framed<TcpStream, Codec>, player: &Player) {
    send_packet(
        framed,
        SpawnPlayer {
            entity_id: player.entity_id,
            uuid: player.uuid,
            //username: p.username.clone(),
            properties: player.properties.clone(),
            x: player.x,
            y: player.y,
            z: player.z,
            yaw: player.yaw,
            pitch: player.pitch,
            current_item: 0,
        },
    )
    .await;
    send_packet(
        framed,
        EntityMetadata {
            entity_id: player.entity_id,
            entity_flags: player.entity_flags(),
            skin_parts: player.skin_parts,
        },
    )
    .await;

    send_player_equipment(
        framed,
        player.entity_id,
        player.held_item_id,
        player.helmet,
        player.chestplate,
        player.leggings,
        player.boots,
    )
    .await;
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
    generator: &Arc<FlatWorldGenerator>,
    entity_tracker: &Arc<RwLock<EntityTracker>>,
    config: &Config,
    spawn_point: &Arc<RwLock<(f64, f64, f64)>>,
    world_dir: &Path,
) {
    if config.server.compression_threshold >= 0 {
        send_packet(
            framed,
            SetCompression {
                threshold: config.server.compression_threshold,
            },
        )
        .await;

        framed.codec_mut().compression_threshold = config.server.compression_threshold;
    }

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
            PlayerListItemAdd {
                uuid,
                username: profile.username.clone(),
                properties: profile.properties.clone(),
                gamemode: config.server.default_gamemode as i32,
                ping: state.latency_ms as i32,
            },
        )
        .await;

        let already_online = player_registry.get_all().await;

        for p in &already_online {
            if p.uuid == uuid {
                continue;
            }
            send_packet(
                framed,
                PlayerListItemAdd {
                    uuid: p.uuid,
                    username: p.username.clone(),
                    properties: p.properties.clone(),
                    gamemode: p.gamemode as i32,
                    ping: p.latency_ms as i32,
                },
            )
            .await;
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

    let saved = load_player_data(world_dir, &uuid).await;
    let (px, py, pz, pyaw, ppitch, phealth, pfood, psat, pgm) = if let Some(d) = saved {
        (
            d.x,
            d.y,
            d.z,
            d.yaw,
            d.pitch,
            d.health,
            d.food,
            d.food_saturation,
            d.gamemode,
        )
    } else {
        let (sx, sy, sz) = *spawn_point.read().await;
        (
            sx,
            sy,
            sz,
            90.0,
            0.0,
            20.0,
            20,
            5.0,
            config.server.default_gamemode,
        )
    };

    if client_protocol == 47 {
        send_packet(
            framed,
            SpawnPosition {
                x: px as i32,
                y: py as i32,
                z: pz as i32,
            },
        )
        .await;
    } else {
        send_packet(
            framed,
            SpawnPosition17 {
                x: px as i32,
                y: py as i32,
                z: pz as i32,
            },
        )
        .await;
    }

    let (ability_flags, fly_speed, walk_speed) = match config.server.default_gamemode {
        1 => (0x01 | 0x02 | 0x04 | 0x08, 0.05, 0.1),
        3 => (0x01 | 0x02 | 0x04, 0.05, 0.1),
        _ => (0x00, 0.05, 0.1),
    };

    send_packet(
        framed,
        PlayerAbilities {
            flags: ability_flags,
            fly_speed,
            walk_speed,
        },
    )
    .await;

    send_chunks(
        framed,
        client_protocol,
        world_blocks,
        generator,
        0,
        0,
        config.server.view_distance,
        &mut state.loaded_chunks,
    )
    .await;

    send_packet(
        framed,
        PlayerPositionAndLook {
            x: px,
            y: py,
            z: pz,
            yaw: pyaw,
            pitch: ppitch,
            on_ground: false,
        },
    )
    .await;

    let player = Player::new(
        entity_id,
        uuid,
        profile.username.clone(),
        profile.properties.clone(),
        px,
        py,
        pz,
        pyaw,
        ppitch,
        pgm,
        phealth,
        pfood,
        psat,
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
        px,
        py,
        pz
    );

    let existing_players = player_registry.get_all().await;
    for p in existing_players {
        if p.uuid == uuid {
            continue;
        }
        send_spawn_player(framed, &p).await;
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
            health: phealth,
            food: pfood,
            food_saturation: psat,
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
    state.gamemode = pgm;
    state.health = phealth;
    state.food = pfood;
    state.food_saturation = psat;

    state.chunk_x = (px as i32) >> 4;
    state.chunk_z = (pz as i32) >> 4;

    entity_tracker.write().await.track(TrackedEntity::player(
        entity_id,
        uuid,
        px,
        py,
        pz,
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
