use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use std::vec;

use bytes::{Buf, Bytes, BytesMut};
use coral_server::effects::ActiveEffect;
use coral_server::items::ItemRegistry;
use coral_server::ops::OpsFile;
use coral_server::projectile::Projectile;
use coral_types::GameMode;
use coral_world::generator::FlatWorldGenerator;
use coral_world::playerdata::load_player_data;
use futures::SinkExt;
use tokio::net::TcpStream;
use tokio::sync::{RwLock, broadcast::Sender};
use tokio_util::codec::{Decoder, Encoder, Framed};
use uuid::Uuid;

use crate::codec::state::play::{self, send_chunks, send_spawn_player, send_weather};
use crate::codec::state::preplay::{JoinRequest, PrePlayContext};
use crate::codec::state::throttle;
use crate::{EquipmentUpdate, JoinLeave, ServerContext};
use coral_config::Config;
use coral_protocol::encryption::Encryption;
use coral_protocol::packets::login::SetCompression;
use coral_protocol::packets::login::disconnect::{LoginDisconnect, PlayDisconnect};
use coral_protocol::packets::play::chat::builder::ChatBuilder;
use coral_protocol::packets::play::chat::builder::ChatColor;
use coral_protocol::packets::play::entity::EntityMetadata;
use coral_protocol::packets::play::game::{ChangeGameState, SetExperience, UpdateHealth};
use coral_protocol::packets::play::inventory::{Inventory, WindowItems};
use coral_protocol::packets::play::movement::PlayerPositionAndLook;
use coral_protocol::packets::play::player_list::{PlayerListItem17, PlayerListItemAdd};
use coral_protocol::packets::{PacketIn, PacketOut};
use coral_protocol::{
    packets::{
        PacketKey, PacketRegistry,
        handshake::{self, EnumProtocol},
        login::LoginSuccess,
        play::{PlayerAbilities, SpawnPosition, SpawnPosition17, join_game::JoinGame},
    },
    reader::Reader,
    writer::Writer,
};
use coral_server::{
    entity_tracker::{EntityTracker, TrackedEntity},
    player::Player,
    registry::{PlayerRegistry, next_entity_id},
};
use coral_world::{blocks::WorldBlocks, weather::WeatherState};

mod state;

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

        let body = if self.compression_threshold >= 0 {
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

struct PlayerState {
    uuid: Option<Uuid>,
    entity_id: i32,
    gamemode: GameMode,
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
    latency_ms: (i32, i32),
    name: Option<String>,
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
    is_op: bool,
    first_position_received: bool,
}
impl PlayerState {
    fn new(default_gamemode: u8) -> Self {
        Self {
            uuid: None,
            entity_id: 0,
            gamemode: GameMode::try_from(default_gamemode).unwrap_or(GameMode::Survival),
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
            latency_ms: (0, 0),
            name: None,
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
            is_op: false,
            first_position_received: false,
        }
    }

    fn get_equipped_armor(&self) -> (i16, i16, i16, i16) {
        let slot = |i: usize| -> i16 { self.inventory.slots[i].as_ref().map_or(-1, |s| s.item_id) };
        (slot(5), slot(6), slot(7), slot(8))
    }

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
        equip_tx: &Arc<Sender<EquipmentUpdate>>,
    ) {
        let (helmet, chest, legs, boots) = self.get_equipped_armor();

        if let Some(uuid) = self.uuid {
            player_registry
                .update_armor(uuid, helmet, chest, legs, boots)
                .await;
        }

        for (slot, item) in [(1, boots), (2, legs), (3, chest), (4, helmet)] {
            equip_tx.send((self.entity_id, slot, item, 1, 0)).ok();
        }
    }

    async fn try_retract_fishing_hook(
        &mut self,
        projectiles: &RwLock<Vec<Projectile>>,
        despawn_tx: &Sender<i32>,
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

    #[allow(clippy::too_many_arguments)]
    async fn damage_player(
        &mut self,
        framed: &mut Framed<TcpStream, Codec>,
        amount: f32,
        player_registry: &Arc<PlayerRegistry>,
    ) -> bool {
        self.health = (self.health - amount).max(0.0);

        player_registry
            .update_health(
                self.uuid.unwrap_or_default(),
                self.health,
                self.food,
                self.food_saturation,
            )
            .await;

        send_packet(
            framed,
            UpdateHealth {
                health: self.health,
                food: self.food,
                food_saturation: self.food_saturation,
            },
        )
        .await;

        if self.health <= 0.0 {
            self.is_dead = true;
            return true;
        }
        false
    }
}

pub async fn process(socket: TcpStream, ctx: ServerContext) {
    let codec = Codec {
        registry: ctx.packet_registry.clone(),
        state: EnumProtocol::Handshaking,
        encryption: None,
        compression_threshold: -1,
        decrypted_buf: BytesMut::new(),
    };
    let peer_ip = socket.peer_addr().ok();
    let mut state = PlayerState::new(ctx.config.server.default_gamemode);
    let mut framed = Framed::new(socket, codec);

    let Some(req) = state::preplay::pre_play(
        &mut framed,
        PrePlayContext {
            player_registry: ctx.player_registry.clone(),
            config: ctx.config.clone(),
            server_icon: ctx.server_icon.clone(),
            banlist: ctx.banlist.clone(),
            whitelist: ctx.whitelist.clone(),
            ops: ctx.ops.clone(),
            private_key: ctx.private_key.clone(),
            public_key_der: ctx.public_key_der.clone(),
            channels: ctx.channels.clone(),
            bungee_adresses: Arc::new(ctx.config.bungee.addresses.clone()),
            connection_throttle: Arc::new(throttle::ConnectionThrottle::new(
                ctx.config.server.connection_throttle_ms,
            )),
        },
        peer_ip,
    )
    .await
    else {
        return;
    };

    let client_protocol = req.client_protocol;

    make_player_join(
        &mut framed,
        &mut state,
        req,
        &ctx.player_registry,
        &ctx.channels.join_tx,
        &ctx.channels.chat_tx,
        &ctx.world_blocks,
        &ctx.generator,
        &ctx.entity_tracker,
        &ctx.config,
        &ctx.spawn_point,
        &ctx.world_dir,
        &ctx.ops,
    )
    .await;

    play::play(&mut framed, &mut state, ctx, client_protocol).await;
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

#[allow(clippy::too_many_arguments)]
async fn make_player_join(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    req: JoinRequest,
    player_registry: &Arc<PlayerRegistry>,
    join_tx: &Arc<Sender<JoinLeave>>,
    chat_tx: &Arc<Sender<String>>,
    world_blocks: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
    entity_tracker: &Arc<RwLock<EntityTracker>>,
    config: &Config,
    spawn_point: &Arc<RwLock<(f64, f64, f64)>>,
    world_dir: &Path,
    ops: &Arc<RwLock<OpsFile>>,
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

    let JoinRequest {
        uuid,
        profile,
        client_protocol,
        peer_ip,
    } = req;

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
            max_player: config.server.max_players as u8,
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
                ping: state.latency_ms.0,
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
                    ping: p.latency_ms,
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
                ping: state.latency_ms.0 as i16,
            },
        )
        .await;
    }

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
            GameMode::try_from(d.gamemode).unwrap_or(GameMode::Survival),
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
            GameMode::try_from(config.server.default_gamemode).unwrap_or(GameMode::Survival),
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

    state.chunk_x = (px as i32) >> 4;
    state.chunk_z = (pz as i32) >> 4;

    send_chunks(
        framed,
        client_protocol,
        world_blocks,
        generator,
        state.chunk_x,
        state.chunk_z,
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
            // TODO: complete save and load to player world files
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
    state.is_op = ops.read().await.is_op(uuid);

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
