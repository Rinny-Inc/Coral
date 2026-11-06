use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering::Relaxed},
    time::{Duration, Instant},
};

use coral_protocol::packets::play::{
    ClientSettings, NamedSoundEffect, PlayerAbilities, PluginMessage, ResourcePackStatus,
    block::{
        BlockAction, BlockBreakAnimation, BlockChange, HeldItemChange, ItemEntityMetadata,
        PlayerBlockPlacement, PlayerDig,
    },
    chat::{
        ChatMessage, ChatMessageOut, TabComplete,
        builder::{ChatAppender, ChatBuilder, ChatColor},
    },
    entity::{
        ArmAnimation, CollectItem, DestroyEntities, EntityAction, EntityAnimation,
        EntityAnimationType, EntityEquipment, EntityHeadLook, EntityLook, EntityLookAndMove,
        EntityMetadata, EntityRelativeMove, EntityTeleport, EntityVelocity, MetadataValue,
        SpawnExperienceOrb, SpawnObject, SpawnPlayer, TileEntity, UpdateSign, UseBed, UseEntity,
    },
    game::{
        ChangeGameState, ClientStatus, EntityStatus, EntityStatusType, GameStateChangeReason,
        ServerDifficulty, SetExperience, UpdateHealth,
    },
    inventory::{
        ClickWindow, CloseWindow, ConfirmTransaction, CreativeInventoryAction, Inventory,
        ItemStack, WindowItems, WindowProperty, WindowType,
    },
    keepalive::KeepAlive,
    movement::{
        MoveKind, MovementBroadcast, PlayerLook, PlayerMovements, PlayerOnGround, PlayerPosition,
        PlayerPositionAndLook,
    },
    player_list::{PlayerListItem17, PlayerListItemAdd, PlayerListItemRemove, UpdateLatency},
};
use coral_server::{
    effects::{ActiveEffect, EffectKind},
    entity_tracker::TrackedEntity,
    experience::{self},
    items::{
        drop_physics::manual_drop_velocity, drops::DroppedItems, ids::EMPTY_HAND,
        potions::PotionEffect,
    },
    player::{
        Player,
        registry::{PlayerRegistry, next_entity_id},
    },
    tile_entities::TileEntityStore,
};
use coral_types::{GameMode, dist_xz};
use coral_world::{
    chunk::{ChunkData, UnloadChunk},
    playerdata::{PlayerData, save_player_data},
    spawn,
    time::TimeUpdate,
    weather::WeatherState,
    world::World,
};
use tokio::{
    net::TcpStream,
    sync::{RwLock, broadcast::Sender},
    time::interval,
};
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

use crate::{
    ServerContext, SoundEffect,
    codec::{
        Codec, PlayerState, is_normal_disconnect, kick, send_packet,
        state::play::{
            handlers::{chat, entity, inventory},
            tile_entity_window::slot_tuple,
            window_ops::close_tile_entity_window,
        },
    },
};
use handlers::{PlayCtx, flow, movement as movement_handler, session, world as world_handler};

mod handlers;
mod interact;
pub mod inventory_click;
mod ticking;
pub mod tile_entity_window;
mod window_ops;

/// How often does the server probes a connection
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

/// What to do when the probe timer fires
#[derive(Debug, PartialEq, Eq)]
enum KeepAliveAction {
    /// The last probe was answered; send another
    Probe,
    /// It was not so the connection is gone
    Timeout,
}

/// Decide from the outstanding probe if any
fn keep_alive_action(outstanding: Option<(i32, std::time::Instant)>) -> KeepAliveAction {
    match outstanding {
        Some(_) => KeepAliveAction::Timeout,
        None => KeepAliveAction::Probe,
    }
}

async fn refuse_out_of_reach(
    framed: &mut Framed<TcpStream, Codec>,
    world: &Arc<World>,
    name: &str,
    x: i32,
    y: i32,
    z: i32,
) {
    tracing::debug!("{name} tried to interact with ({x}, {y}, {z}) from out of reach");

    if !(0..=255).contains(&y) {
        return;
    }
    let block = world.get(x, y as u8, z).await;
    send_packet(
        framed,
        BlockChange {
            x,
            y,
            z,
            block_id: block.id as i32,
            block_metadata: block.metadata,
        },
    )
    .await;
}

macro_rules! decode_or_disconnect {
    ($frame:expr, $ty:ty) => {
        match $frame.decode::<$ty>() {
            Ok(decoded) => decoded,
            Err(e) => {
                tracing::error!(
                    "Malformed {} (id 0x{:02X}): {e:?}",
                    stringify!($ty),
                    $frame.id
                );
                break;
            }
        }
    };
}

pub async fn play(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: ServerContext,
    client_protocol: i32,
) {
    let ServerContext {
        player_registry,
        item_registry,
        block_registry,
        config,
        dispatcher,
        entity_tracker,
        item_spawn_times,
        item_positions,
        projectiles,
        channels,
        world,
        spawn_point,
        world_dir,
        xp_orbs,
        fluid_queue,
        tile_entities,
        server_loaded_chunks,
        ..
    } = ctx;

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
    let mut xp_pickup_rx = channels.xp_pickup_tx.subscribe();
    let mut xp_orb_spawn_rx = channels.xp_orb_spawn_tx.subscribe();
    let mut xp_orb_move_rx = channels.xp_orb_move_tx.subscribe();
    let mut bed_rx = channels.bed_tx.subscribe();
    let mut wake_rx = channels.wake_tx.subscribe();
    let mut private_msg_rx = channels.private_msg_tx.subscribe();
    let mut teleport_rq_rx = channels.teleport_rq_tx.subscribe();
    let mut kick_rq_rx = channels.kick_rq_tx.subscribe();
    let mut shutdown_rx = channels.shutdown_tx.subscribe();
    let mut sign_update_rx = channels.sign_update_tx.subscribe();
    let mut velocity_broadcast_rx = channels.velocity_broadcast_tx.subscribe();
    let mut chest_anim_rx = channels.chest_anim_tx.subscribe();
    let mut furnace_update_rx = channels.furnace_update_tx.subscribe();
    let mut difficulty_rx = channels.difficulty_tx.subscribe();

    let mut keep_alive_interval = interval(KEEP_ALIVE_INTERVAL); // 30 seconds is timed out

    let ctx = PlayCtx {
        player_registry: &player_registry,
        item_registry: &item_registry,
        config: &config,
        channels: &channels,
        projectiles: &projectiles,
        tile_entities: &tile_entities,
        entity_tracker: &entity_tracker,
        item_positions: &item_positions,
        item_spawn_times: &item_spawn_times,
        dispatcher: &dispatcher,
        world: &world,
        spawn_point: &spawn_point,
        server_loaded_chunks: &server_loaded_chunks,
        xp_orbs: &xp_orbs,
        block_registry: &block_registry,
        fluid_queue: &fluid_queue,
        client_protocol,
    };

    loop {
        tokio::select! {
            _ = keep_alive_interval.tick() => {
                if keep_alive_action(state.last_sent_keep_alive) == KeepAliveAction::Timeout {
                    if let Some((unanswered, sent_at)) = state.last_sent_keep_alive {
                        tracing::info!("{} timed out after {:.0}s without answering keepalive {unanswered}", state.name, sent_at.elapsed().as_secs_f32());
                    }
                    kick(framed, "Timed out").await;
                    break;
                }
                state.keep_alive_count += 1;
                state.last_sent_keep_alive = Some((state.keep_alive_count, std::time::Instant::now()));
                send_packet(framed, KeepAlive { id: state.keep_alive_count }).await;
            }
            Ok(is_restarting) = shutdown_rx.recv() => {
                if is_restarting {
                    kick(framed, "Server restarting.").await;
                } else {
                    kick(framed, "Server closed.").await;
                }
                break;
            }
            Ok(()) = tick_rx.recv() => {
                state.tick_count += 1;

                if state.is_dead {
                    continue;
                }
                ticking::handle_tick(framed,
                    state,
                    &player_registry,
                    &item_registry,
                    &config,
                    &item_spawn_times,
                    &item_positions,
                    &entity_tracker,
                    &world,
                    &channels,
                ).await;
            }
            Ok((eid, x, y, z)) = bed_rx.recv() => {
                if eid == state.entity_id || !state.has_chunk_at(x, z) {
                    continue;
                }
                send_packet(framed, UseBed {
                    entity_id: eid,
                    x, y, z
                }).await;
            }
            Ok(()) = wake_rx.recv() => {
                if !state.is_sleeping {
                    continue;
                }
                state.is_sleeping = false;
                send_packet(framed, EntityAnimation {
                    entity_id: state.entity_id,
                    animation: EntityAnimationType::LeaveBed,
                }).await;
            }
            Ok((sender_eid, _id, x, _y, z, _ox, _oy, _oz, _data, _count, _extra)) = particle_rx.recv() => {
                if state.entity_id == sender_eid || !state.has_chunk_at_pos(x as f64, z as f64) {
                    continue;
                }
                /*send_packet(framed, WorldParticles {
                    particle_id: id,
                    long_distance: false,
                    x, y, z,
                    offset_x: ox,
                    offset_y: oy,
                    offset_z: oz,
                    particle_data: data,
                    count,
                    extra
                }).await;*/
            }
            Ok((from, to, message)) = private_msg_rx.recv() => {
                if state.name != to {
                    continue;
                }

                state.last_message_from = Some(from.clone());

                let json = ChatAppender::new()
                    .push(ChatBuilder::new(format!("{} -> You: ", from))
                        .color(ChatColor::Gray).italic())
                    .push(ChatBuilder::new(&message).color(ChatColor::Gray).italic())
                    .build();
                send_packet(framed, ChatMessageOut::from_json(&json)).await;
            }
            Ok((x, y, z, viewers)) = chest_anim_rx.recv() => { // TODO: add block_id
                if state.loaded_chunks.contains(&((x >> 4), (z >> 4))) {
                    send_packet(framed, BlockAction {
                        x, y, z,
                        action_id: 1,
                        action_param: viewers,
                        block_type: 54, // TODO: do I need trapped chest id??
                    }).await;
                }
            }
            Ok((target_uuid, effect_id, amplifier, duration)) = splash_effect_rx.recv() => {
                if target_uuid != state.uuid {
                    continue;
                }

                let Some(kind) = EffectKind::from_id(effect_id) else {
                    continue;
                };

                let pe = PotionEffect { kind, amplifier, duration_ticks: duration };
                apply_potion_effect(framed, state, &player_registry, pe).await;
            }
            Ok((eid, owner_eid, kind, x, y, z, vx, vy, vz)) = projectile_spawn_rx.recv() => {
                let object_type = kind.entity_id();

                send_packet(framed, SpawnObject {
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
                if !state.has_chunk_at_pos(x, z) {
                    continue;
                }
                send_packet(framed, EntityTeleport {
                    entity_id: eid,
                    x, y, z,
                    yaw: 0.0, pitch: 0.0,
                    on_ground: false
                }).await;
            }
            Ok(weather) = weather_rx.recv() => {
                state.current_weather = weather.clone();
                send_weather(framed, weather).await;
            }
            Ok(entity_ids) = despawn_rx.recv() => {
                send_packet(framed, DestroyEntities {
                    entity_ids
                }).await;
            }
            Ok((eid, status)) = status_rx.recv() => {
                send_packet(framed, EntityStatus {
                    entity_id: eid,
                    status,
                }).await;
            }
            Ok(difficulty) = difficulty_rx.recv() => {
                send_packet(framed, ServerDifficulty { difficulty }).await;
            }
            Ok((sound, x, y, z, volume, pitch)) = sound_rx.recv() => {
                if !state.has_chunk_at_pos(x, z) {
                    continue;
                }
                // TODO: filter sounds that are sent client side & skip sound maker
                send_packet(framed, NamedSoundEffect {
                    sound: sound.to_string(), x, y, z, volume, pitch
                }).await;
            }
            Ok((collector_eid, _collector_uuid, item_eid)) = pickup_rx.recv() => {
                send_packet(framed, CollectItem {
                    collected_entity_id: item_eid,
                    collector_entity_id: collector_eid,
                }).await;
                send_packet(framed, DestroyEntities {
                    entity_ids: vec![item_eid]
                }).await;
            }
            Ok((world_age, time_of_day)) = time_rx.recv() => {
                world.time.store(time_of_day, Relaxed);
                send_packet(framed, TimeUpdate {
                    world_age,
                    time_of_day
                }).await;
            }
            Ok((eid, slot, item_id, count, metadata)) = equip_rx.recv() => {
                if eid == state.entity_id {
                    continue;
                }
                send_packet(framed, EntityEquipment {
                    entity_id: eid,
                    slot,
                    item_id,
                    count,
                    metadata
                }).await;
            }
            Ok((eid, vx, vy, vz)) = velocity_broadcast_rx.recv() => {
                if eid == state.entity_id {
                    continue;
                }
                send_packet(framed, EntityVelocity {
                    entity_id: eid,
                    vx, vy, vz
                }).await;
            }
            Ok(pos) = furnace_update_rx.recv() => {
                if let Some(WindowType::Furnace { window_id, pos: open_pos }) = &state.open_window
                    && *open_pos == pos
                {
                    let window_id = *window_id;
                    if let Some(TileEntity::Furnace { input, fuel, output, burn_ticks, burn_ticks_total, cook_ticks, .. }) =
                        tile_entities.read().await.get(&pos)
                    {
                        send_packet(framed, WindowProperty { window_id, property: 0, value: *burn_ticks }).await;
                        send_packet(framed, WindowProperty { window_id, property: 1, value: (*burn_ticks_total).max(1) }).await;
                        send_packet(framed, WindowProperty { window_id, property: 2, value: *cook_ticks }).await;
                        send_packet(framed, WindowProperty { window_id, property: 3, value: 200 }).await;

                        let slots = vec![slot_tuple(input), slot_tuple(fuel), slot_tuple(output)];
                        let mut all_slots = slots;
                        for i in 9..36 {
                            all_slots.push(slot_tuple(&state.inventory.slots[i]));
                        }
                        for i in 0..9 {
                            all_slots.push(slot_tuple(&state.inventory.slots[i]));
                        }
                        send_packet(framed, WindowItems { window_id, slots: all_slots }).await;
                    }
                }
            }

            Ok(mv) = pos_rx.recv() => {
                if mv.entity_id == state.entity_id {
                    continue;
                }
                let (my_x, _, my_z) = state.last_position;
                if !entity_tracker.read().await.is_visible_to(mv.entity_id, my_x, my_z) {
                    continue;
                }

                // particles
                /*let was_on_ground = player_registry.get(&state.uuid).await
                    .map(|p| p.on_ground)
                    .unwrap_or(true);

                    if state.was_on_ground && !was_on_ground {
                        let land_block = world.get(
                            my_x.floor() as i32,
                            (my_y.floor() as i32 - 1).max(0) as u8,
                            my_z.floor() as i32,
                        ).await;

                        if !land_block.is_air() {
                            channels.particle_tx.send((
                                state.entity_id,
                                37,
                                my_x as f32,
                                my_y as f32,
                                my_z as f32,
                                0.3, 0.0, 0.3,
                                land_block.id as f32,
                                6,
                                [land_block.id as i32 | ((land_block.metadata as i32) << 12), 0]
                            )).ok();
                        }
                    }

                    let is_sprinting = player_registry.get(&state.uuid).await
                        .map(|p| p.is_sprinting)
                        .unwrap_or(false);

                    if is_sprinting && state.was_on_ground {
                        let block_below = world.get(
                            my_x.floor() as i32,
                            (my_y.floor() as i32 - 1).max(0) as u8,
                            my_z.floor() as i32,
                        ).await;

                        if !block_below.is_air() {
                            let Some(yaw) = mv.head_yaw else {
                                continue;
                            };
                            let yaw_rad = (yaw * std::f32::consts::PI / 180.0) as f64;
                            let behind_x = my_x + yaw_rad.sin() * 0.2;
                            let behind_z = my_z - yaw_rad.cos() * 0.2;

                            channels.particle_tx.send((
                                state.entity_id,
                                37,
                                behind_x as f32,
                                my_y as f32,
                                behind_z as f32,
                                0.0, 0.0, 0.0,
                                block_below.id as f32,
                                3,
                                [block_below.id as i32 | ((block_below.metadata as i32) << 12), 0]
                            )).ok();
                        }
                    }*/
                match mv.kind {
                    MoveKind::Relative { dx, dy, dz, on_ground } => {
                        send_packet(framed, EntityRelativeMove {
                            entity_id: mv.entity_id, dx, dy, dz, on_ground,
                        }).await; // 0x15
                    }
                    MoveKind::Look { yaw, pitch, on_ground } => {
                        send_packet(framed, EntityLook {
                            entity_id: mv.entity_id,
                            yaw,
                            pitch,
                            on_ground,
                        }).await; // 0x16
                    }
                    MoveKind::LookAndRelative { dx, dy, dz, yaw, pitch, on_ground } => {
                        send_packet(framed, EntityLookAndMove {
                            entity_id: mv.entity_id, dx, dy, dz,
                            yaw,
                            pitch,
                            on_ground,
                        }).await; // 0x17
                    }
                    MoveKind::Teleport { x, y, z, yaw, pitch, on_ground } => {
                        send_packet(framed, EntityTeleport {
                            entity_id: mv.entity_id, x, y, z,
                            yaw,
                            pitch,
                            on_ground,
                        }).await; // 0x18
                    }
                }

                if let Some(yaw) = mv.head_yaw {
                    send_packet(framed, EntityHeadLook {
                        entity_id: mv.entity_id,
                        head_yaw: yaw,
                    }).await; // 0x19
                }
            }

            Ok((target_uuid, x, y, z)) = teleport_rq_rx.recv() => {
                if target_uuid != state.uuid {
                    continue;
                }

                state.await_teleport(x, y, z);
                send_packet(framed, PlayerPositionAndLook {
                    x, y, z,
                    yaw: 0.0, pitch: 0.0,
                    on_ground: false,
                }).await;

                player_registry.update_position(&target_uuid, x, y, z, 0.0, 0.0, false).await;
                channels.pos_tx.send(MovementBroadcast {
                    uuid: target_uuid,
                    entity_id: state.entity_id,
                    kind: MoveKind::Teleport { x, y, z, yaw: 0.0, pitch: 0.0, on_ground: false },
                    head_yaw: Some(0.0),
                }).ok();

                let (new_cx, new_cz) = spawn::spawn_chunk(x, z);
                if new_cx != state.chunk_x || new_cz != state.chunk_z {
                    state.chunk_x = new_cx;
                    state.chunk_z = new_cz;
                    update_chunks(framed, client_protocol, &world, &tile_entities, &server_loaded_chunks, new_cx, new_cz, config.server.view_distance, &mut state.loaded_chunks).await;
                }

                state.was_on_ground = false;
            }
            Ok((target_uuid, reason)) = kick_rq_rx.recv() => {
                if target_uuid != state.uuid {
                    continue;
                }
                kick(framed, &format!("§c{}", reason)).await;
                break;
            }

            Ok((uuid, amount)) = xp_pickup_rx.recv() => {
                if uuid != state.uuid {
                    continue;
                }
                state.xp_total += amount;
                let (level, progress) = experience::xp_to_level(state.xp_total);
                state.xp_level = level;
                state.xp_progress = progress;
                player_registry.update_experience(&state.uuid, state.xp_total, state.xp_level).await;

                send_packet(framed, SetExperience {
                    experience_bar: progress,
                    level,
                    total_experience: state.xp_total
                }).await;

                let (sx, sy, sz) = state.last_position;
                channels.sound_tx.send(("random.orb", sx, sy, sz, 0.5, 63)).ok();
            }
            Ok((eid, x, y, z, amount)) = xp_orb_spawn_rx.recv() => {
                let (my_x, _, my_z) = state.last_position;
                if !entity_tracker.read().await.is_visible_to(eid, my_x, my_z) {
                    continue;
                }
                send_packet(framed, SpawnExperienceOrb {
                    entity_id: eid,
                    x, y, z,
                    count: amount as i16
                }).await;
            }
            Ok((eid, x, y, z)) = xp_orb_move_rx.recv() => {
                let (my_x, _, my_z) = state.last_position;
                if !entity_tracker.read().await.is_visible_to(eid, my_x, my_z) {
                    continue;
                }
                send_packet(framed, EntityTeleport {
                    entity_id: eid,
                    x, y, z,
                    yaw: 0.0, pitch: 0.0,
                    on_ground: false
                }).await;
            }

            Ok((eid, anim)) = anim_rx.recv() => {
                if eid == state.entity_id {
                    continue;
                }
                send_packet(framed, EntityAnimation {
                    entity_id: eid,
                    animation: anim
                }).await;
            }
            Ok(update) = meta_rx.recv() => {
                if update.entity_id == state.entity_id {
                    continue;
                }
                send_packet(framed, update).await;
            }
            Ok((x, y, z, block_id, metadata)) = block_rx.recv() => {
                if !state.has_chunk_at(x, z) {
                    continue;
                }
                send_packet(framed, BlockChange {
                    x, y, z,
                    block_id,
                    block_metadata: metadata
                }).await;
            }
            Ok((eid, x, y, z, stage)) = break_rx.recv() => {
                if eid == state.entity_id || !state.has_chunk_at(x, z) {
                    continue;
                }
                send_packet(framed, BlockBreakAnimation {
                    entity_id: eid,
                    x, y, z,
                    destroy_stage: stage
                }).await;
            }
            Ok((player, join_event)) = join_rx.recv() => {
                if player.uuid == state.uuid {
                    continue;
                }
                if !join_event {
                    send_packet(framed, DestroyEntities {
                        entity_ids: vec![player.entity_id]
                    }).await;
                    send_packet(framed, PlayerListItemRemove {
                        uuid: player.uuid
                    }).await;
                } else {
                    if client_protocol == 47 {
                        send_packet(framed, PlayerListItemAdd {
                            uuid: player.uuid,
                            username: player.username.to_string(),
                            properties: player.properties.to_vec(),
                            gamemode: player.gamemode as i32,
                            ping: player.latency_ms
                        }).await;
                    } else {
                        send_packet(framed, PlayerListItem17 {
                            username: player.username.to_string(),
                            online: true,
                            ping: player.latency_ms as i16
                        }).await;
                    }
                    if let Some(me) = player_registry.get(&state.uuid).await
                        && dist_xz(player.x, player.z, me.x, me.z) > config.tracking.player
                    {
                        continue;
                    }
                    send_spawn_player(framed, &player).await;
                }
            }
            Ok((x, y, z, lines)) = sign_update_rx.recv() => {
                if !(state.has_chunk_at(x, z)) {
                    continue;
                }
                send_packet(framed, UpdateSign { x, y, z, lines }).await;
            }
            Ok((uuid, health, food, food_saturation, attacker_eid)) = dmg_rx.recv() => {
                if uuid != state.uuid {
                    continue;
                }
                state.health = health;
                state.food = food;
                state.food_saturation = food_saturation;
                state.is_dead = health <= 0.0;
                if state.is_dead {
                    state.clear_experience(framed, &player_registry).await;
                }

                send_packet(framed, UpdateHealth {
                    health,
                    food,
                    food_saturation,
                }).await;

                let status = if state.is_dead {
                    EntityStatusType::DeadAnimation
                } else {
                    EntityStatusType::HurtAnimation
                };
                send_packet(framed, EntityStatus {
                    entity_id: state.entity_id,
                    status,
                }).await;

                if health > 0.0
                    && let Some(me) = player_registry.get(&uuid).await
                    && let Some(attacker) = player_registry.get_by_entity_id(attacker_eid).await
                {
                    let magnitude = dist_xz(me.x, me.z, attacker.x, attacker.z).max(0.0001);
                    let mut vx = ((me.x - attacker.x) / magnitude) * 0.4;
                    let mut vz = ((me.z - attacker.z) / magnitude) * 0.4;
                    let vy = 0.4;

                    if attacker.is_sprinting {
                        let yaw_rad = (attacker.yaw * std::f32::consts::PI / 180.0) as f64;
                        vx += -yaw_rad.sin() * 0.4;
                        vz +=  yaw_rad.cos() * 0.4;
                    }

                    send_packet(framed, EntityVelocity {
                        entity_id: state.entity_id,
                        vx,
                        vy,
                        vz,
                    }).await;

                    state.velocity = (vx, vy, vz);
                    state.last_position_tick = Instant::now();
                    player_registry.update_velocity(&uuid, state.velocity).await;
                }
            }
            Ok((eid, x, y, z, item_id, count, metadata, vx, vy, vz)) = item_rx.recv() => {
                let (my_x, _, my_z) = state.last_position;
                if !entity_tracker.read().await.is_visible_to(eid, my_x, my_z) {
                    continue;
                }
                let v_max = i16::MAX as f64;
                let v_min = i16::MIN as f64;
                send_packet(framed, SpawnObject {
                    entity_id: eid,
                    object_type: 2, // itemstack
                    x, y, z,
                    yaw: 0,
                    pitch: 0,
                    data: 1, // non zero to send velocity
                    vx: (vx * 8000.0).clamp(v_min, v_max) as i16,
                    vy: (vy * 8000.0).clamp(v_min, v_max) as i16,
                    vz: (vz * 8000.0).clamp(v_min, v_max) as i16,
                }).await;
                send_packet(framed, ItemEntityMetadata {
                    entity_id: eid,
                    item_id,
                    item_count: count,
                    item_damage: metadata
                }).await;
            }
            Ok(message) = chat_rx.recv() => {
                send_packet(framed, ChatMessageOut::from_json(&message)).await;
            }
            Ok((uuid, ping)) = ping_rx.recv() => {
                send_packet(framed, UpdateLatency {
                    uuid,
                    ping
                }).await;
            }
            Ok((uuid, gamemode)) = gm_rx.recv() => {
                if uuid != state.uuid {
                    continue;
                }
                state.gamemode = gamemode;
                let gm_u8 = u8::from(gamemode);
                send_packet(framed, ChangeGameState::set_gamemode(gm_u8)).await;

                let (flags, fly_speed, walk_speed) = match gm_u8 {
                    1 => (0x01 | 0x02 | 0x04 | 0x08, 0.05, 0.1),
                    3 => (0x01 | 0x02 | 0x04, 0.05, 0.1),
                    _ => (0x00, 0.05, 0.1),
                };
                send_packet(framed, PlayerAbilities {
                    flags,
                    fly_speed,
                    walk_speed
                }).await;
            }
            result = framed.next() => {
                let Some(result) = result else {
                    break
                };
                match result {
                    Ok(packet) => {
                        if packet.is::<KeepAlive>() {
                            let ka = decode_or_disconnect!(packet, KeepAlive);
                            flow!(session::keep_alive(ka, state, &ctx).await);
                        }

                        if packet.is::<HeldItemChange>() {
                            let held = decode_or_disconnect!(packet, HeldItemChange);
                            flow!(session::held_item_change(held, state, &ctx).await);
                        }

                        if packet.is::<PlayerDig>() {
                            let dig = decode_or_disconnect!(packet, PlayerDig);
                            flow!(world_handler::player_dig(dig, framed, state, &ctx).await);
                        }

                        if packet.is::<PlayerBlockPlacement>() {
                            let place = decode_or_disconnect!(packet, PlayerBlockPlacement);
                            flow!(world_handler::player_block_placement(place, framed, state, &ctx).await);
                        }

                        if packet.is::<ArmAnimation>() {
                            flow!(entity::arm_animation(state, &ctx));
                        }

                        if packet.is::<ChatMessage>() {
                            let chat = decode_or_disconnect!(packet, ChatMessage);
                            flow!(chat::chat_message(chat, framed, state, &ctx).await);
                        }
                        if packet.is::<TabComplete>() {
                            let tab = decode_or_disconnect!(packet, TabComplete);
                            flow!(chat::tab_complete(tab, framed, &ctx).await);
                        }

                        let movement: Option<PlayerMovements> = if packet.is::<PlayerPosition>() {
                            Some((&decode_or_disconnect!(packet, PlayerPosition)).into())
                        } else if packet.is::<PlayerLook>() {
                            Some((&decode_or_disconnect!(packet, PlayerLook)).into())
                        } else if packet.is::<PlayerPositionAndLook>() {
                            Some((&decode_or_disconnect!(packet, PlayerPositionAndLook)).into())
                        } else if packet.is::<PlayerOnGround>() {
                            Some((&decode_or_disconnect!(packet, PlayerOnGround)).into())
                        } else {
                            None
                        };

                        if let Some(mv) = movement
                            && let Some(p) = player_registry.get(&state.uuid).await
                        {
                            flow!(movement_handler::player_moved(mv, &p, framed, state, &ctx).await);
                        }

                        if packet.is::<CloseWindow>() {
                            flow!(session::close_window(state, &ctx).await);
                        }

                        if packet.is::<ConfirmTransaction>() {
                            // client acknowledged our confirmation
                            continue;
                        }

                        if packet.is::<ResourcePackStatus>() {
                            let status = decode_or_disconnect!(packet, ResourcePackStatus);
                            flow!(session::resource_pack_status(status, framed, &ctx).await);
                        }

                        if packet.is::<CreativeInventoryAction>() {
                            if state.gamemode != GameMode::Creative {
                                continue;
                            }
                            let creative = decode_or_disconnect!(packet, CreativeInventoryAction);
                            flow!(inventory::creative_inventory_action(creative, state, &ctx).await);
                        }

                        if packet.is::<ClickWindow>() {
                            let click = decode_or_disconnect!(packet, ClickWindow);
                            flow!(inventory::click_window(click, framed, state, &ctx).await);
                        }

                        if packet.is::<ClientStatus>() {
                            let status = decode_or_disconnect!(packet, ClientStatus);
                            flow!(entity::client_status(status, framed, state, &ctx).await);
                        }

                        if packet.is::<EntityAction>() {
                            let action = decode_or_disconnect!(packet, EntityAction);
                            flow!(entity::entity_action(action, framed, state, &ctx).await);
                        }

                        if packet.is::<UseEntity>() {
                            let use_entity = decode_or_disconnect!(packet, UseEntity);
                            flow!(entity::use_entity(use_entity, framed, state, &ctx).await);
                        }

                        if packet.is::<PlayerAbilities>() {
                            let abilities = decode_or_disconnect!(packet, PlayerAbilities);
                            flow!(session::player_abilities(abilities, state));
                        }

                        if packet.is::<UpdateSign>() {
                            let sign = decode_or_disconnect!(packet, UpdateSign);
                            flow!(session::update_sign(sign, state, &ctx).await);
                        }

                        if packet.is::<ClientSettings>() {
                            let settings = decode_or_disconnect!(packet, ClientSettings);
                            flow!(session::client_settings(settings, state, &ctx).await);
                        }

                        if packet.is::<PluginMessage>() {
                            let plugin = decode_or_disconnect!(packet, PluginMessage);
                            flow!(session::plugin_message(plugin, framed, state).await);
                        }

                        tracing::warn!("Unhandled packet: {:?}", packet);
                        continue;
                    }
                    Err(e) => {
                        if !is_normal_disconnect(&e) {
                            tracing::error!("Error processing packet: {:?}", e);
                        }
                        break;
                    }
                }
            }
        }
    }
    if let Some(cursor) = state.cursor_item.take()
        && let Some(overflow) = state.inventory.insert_itemstack(cursor)
    {
        drop_item_in_world(
            overflow,
            state.entity_id,
            &player_registry,
            &channels,
            &item_spawn_times,
            &entity_tracker,
            &item_positions,
            config.tracking.item,
        )
        .await;
    }
    let mut inventory_data = vec![];
    for i in 0..46i16 {
        if let Some(idx) = Inventory::packet_to_internal(i)
            && let Some(slot) = &state.inventory.slots[idx]
        {
            inventory_data.push((i, slot.item_id, slot.count, slot.metadata));
        }
    }
    if let Some(p) = player_registry.get(&state.uuid).await {
        let data = PlayerData {
            x: p.x,
            y: p.y,
            z: p.z,
            yaw: p.yaw,
            pitch: p.pitch,
            health: state.health,
            food: state.food,
            food_saturation: state.food_saturation,
            gamemode: u8::from(state.gamemode),
            inventory: inventory_data,
            held_slot: state.held_slot,
            xp_total: state.xp_total,
            bed_spawn: state.bed_spawn,
            is_flying: state.is_flying,
            fire_ticks: state.fire_ticks,
            air_ticks: state.air_ticks,
            active_effects: state
                .active_effects
                .iter()
                .map(|e| (e.kind.id(), e.amplifier, e.remaining_ticks))
                .collect(),
        };
        save_player_data(&world_dir, &state.uuid, &data).await;
        channels.join_tx.send((p, false)).ok();
    }
    player_registry.remove(&state.uuid).await;
    entity_tracker.write().await.untrack(state.entity_id);
    close_tile_entity_window(
        state,
        &tile_entities,
        &channels.chest_anim_tx,
        &channels.sound_tx,
    )
    .await;
    if !player_registry.is_empty().await {
        channels
            .chat_tx
            .send(ChatBuilder::colored_json(
                &format!("{} left the game", state.name),
                ChatColor::Yellow,
            ))
            .ok();
    }
    tracing::info!(
        "{} left the game; Online: {}",
        state.name,
        player_registry.get_online_count().await
    );
}

async fn spawn_dropped_item(
    p: &coral_server::player::Player,
    stack: ItemStack,
    channels: &crate::Channels,
    item_spawn_times: &Arc<RwLock<HashMap<i32, Instant>>>,
    entity_tracker: &Arc<RwLock<coral_server::entity_tracker::EntityTracker>>,
    item_positions: &Arc<RwLock<DroppedItems>>,
    tracking_range: f64,
) {
    let yaw_rad = p.yaw.to_radians();
    let drop_x = p.x + (-yaw_rad.sin() * 0.3) as f64;
    let drop_y = p.y + 1.2;
    let drop_z = p.z + (yaw_rad.cos() * 0.3) as f64;

    let (vx, vy, vz) = manual_drop_velocity(p.yaw, p.pitch);
    let vx = vx + p.velocity.0 * 0.3;
    let vy = vy + (p.velocity.1 * 0.3).max(0.0);
    let vz = vz + p.velocity.2 * 0.3;
    let drop_eid = next_entity_id();

    entity_tracker.write().await.track(TrackedEntity::item(
        drop_eid,
        drop_x,
        drop_y,
        drop_z,
        tracking_range,
    ));
    item_spawn_times
        .write()
        .await
        .insert(drop_eid, Instant::now());
    item_positions.write().await.insert((
        drop_eid,
        drop_x,
        drop_y,
        drop_z,
        stack.item_id,
        stack.count,
        stack.metadata,
    ));
    channels
        .item_tx
        .send((
            drop_eid,
            drop_x,
            drop_y,
            drop_z,
            stack.item_id,
            stack.count,
            stack.metadata,
            vx,
            vy,
            vz,
        ))
        .ok();
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
            player_registry
                .update_health(&state.uuid, state.health, state.food, state.food_saturation)
                .await;
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
            state.damage_player(framed, dmg, player_registry).await;
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

    player_registry
        .update_effects(&state.uuid, state.active_effects.clone())
        .await;
}
async fn remove_effect(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    kind: EffectKind,
    player_registry: &Arc<PlayerRegistry>,
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

    player_registry
        .update_effects(&state.uuid, state.active_effects.clone())
        .await;
}

pub async fn send_weather(framed: &mut Framed<TcpStream, Codec>, weather: WeatherState) {
    match weather {
        WeatherState::Clear => {
            send_packet(
                framed,
                ChangeGameState {
                    reason: GameStateChangeReason::EndRaining,
                    value: 0.0,
                },
            )
            .await;
        }
        WeatherState::Rain => {
            send_packet(
                framed,
                ChangeGameState {
                    reason: GameStateChangeReason::BeginRaining,
                    value: 0.0,
                },
            )
            .await;
            send_packet(
                framed,
                ChangeGameState {
                    reason: GameStateChangeReason::FadeValue,
                    value: 1.0,
                },
            )
            .await;
            send_packet(
                framed,
                ChangeGameState {
                    reason: GameStateChangeReason::FadeTime,
                    value: 0.0,
                },
            )
            .await;
        }
        WeatherState::Thunder => {
            send_packet(
                framed,
                ChangeGameState {
                    reason: GameStateChangeReason::BeginRaining,
                    value: 0.0,
                },
            )
            .await;
            send_packet(
                framed,
                ChangeGameState {
                    reason: GameStateChangeReason::FadeValue,
                    value: 1.0,
                },
            )
            .await;
            send_packet(
                framed,
                ChangeGameState {
                    reason: GameStateChangeReason::FadeTime,
                    value: 1.0,
                },
            )
            .await;
        }
    }
}

pub async fn send_spawn_player(framed: &mut Framed<TcpStream, Codec>, player: &Player) {
    send_packet(
        framed,
        SpawnPlayer {
            entity_id: player.entity_id,
            uuid: player.uuid,
            //username: p.username.clone(),
            properties: player.properties.to_vec(),
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
            entries: vec![
                (0, MetadataValue::Byte(player.entity_flags())),
                (1, MetadataValue::Short(player.air_tick)),
                (10, MetadataValue::Byte(player.skin_parts)),
            ],
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

fn normalize_yaw(yaw: f32) -> f32 {
    let mut y = yaw % 360.0;
    if y >= 180.0 {
        y -= 360.0;
    } else if y < -180.0 {
        y += 360.0;
    }
    y
}

fn level_to_xp_drop(level: i32) -> i32 {
    level.max(0).saturating_mul(7).min(100)
}

#[allow(clippy::too_many_arguments)]
async fn drop_item_in_world(
    stack: ItemStack,
    entity_id: i32,
    player_registry: &Arc<PlayerRegistry>,
    channels: &crate::Channels,
    item_spawn_times: &Arc<RwLock<HashMap<i32, Instant>>>,
    entity_tracker: &Arc<RwLock<coral_server::entity_tracker::EntityTracker>>,
    item_positions: &Arc<RwLock<DroppedItems>>,
    tracking_range: f64,
) {
    let Some(p) = player_registry.get_by_entity_id(entity_id).await else {
        return;
    };
    let yaw_rad = p.yaw * std::f32::consts::PI / 180.0;
    let drop_x = p.x + (-yaw_rad.sin() * 0.3) as f64;
    let drop_y = p.y + 1.2;
    let drop_z = p.z + (yaw_rad.cos() * 0.3) as f64;

    let (vx, vy, vz) = manual_drop_velocity(p.yaw, p.pitch);
    let vx = vx + p.velocity.0 * 0.3;
    let vy = vy + (p.velocity.1 * 0.3).max(0.0);
    let vz = vz + p.velocity.2 * 0.3;
    let drop_eid = next_entity_id();

    entity_tracker.write().await.track(TrackedEntity::item(
        drop_eid,
        drop_x,
        drop_y,
        drop_z,
        tracking_range,
    ));
    item_spawn_times
        .write()
        .await
        .insert(drop_eid, Instant::now());
    item_positions.write().await.insert((
        drop_eid,
        drop_x,
        drop_y,
        drop_z,
        stack.item_id,
        stack.count,
        stack.metadata,
    ));
    channels
        .item_tx
        .send((
            drop_eid,
            drop_x,
            drop_y,
            drop_z,
            stack.item_id,
            stack.count,
            stack.metadata,
            vx,
            vy,
            vz,
        ))
        .ok();
}

#[allow(clippy::too_many_arguments)]
pub async fn send_chunks(
    framed: &mut Framed<TcpStream, Codec>,
    client_protocol: i32,
    world: &Arc<World>,
    tile_entities: &Arc<RwLock<TileEntityStore>>,
    server_loaded_chunks: &Arc<RwLock<HashSet<(i32, i32)>>>,
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

            let already_loaded = server_loaded_chunks.read().await.contains(&(cx, cz));
            if !already_loaded {
                if let Some(nbt) = world.blocks.get_chunk_nbt(cx, cz).await {
                    for raw in coral_world::anvil::nbt_to_tile_entities(&nbt) {
                        if let Some(tile) = coral_world::anvil::parse_tile_entity(&raw) {
                            tile_entities
                                .write()
                                .await
                                .insert((raw.x, raw.y, raw.z), tile);
                        }
                    }
                }
                server_loaded_chunks.write().await.insert((cx, cz));
            }

            let chunk = ChunkData::build(cx, cz, client_protocol, world).await;
            send_packet(framed, chunk).await;
            loaded_chunks.insert((cx, cz));

            let signs: Vec<UpdateSign> = {
                let te = tile_entities.read().await;
                te.in_chunk(cx, cz)
                    .filter_map(|((x, y, z), tile)| match tile {
                        TileEntity::Sign { lines } => Some(UpdateSign {
                            x: *x,
                            y: *y,
                            z: *z,
                            lines: [
                                ChatBuilder::plain_json(&lines[0]),
                                ChatBuilder::plain_json(&lines[1]),
                                ChatBuilder::plain_json(&lines[2]),
                                ChatBuilder::plain_json(&lines[3]),
                            ],
                        }),
                        _ => None,
                    })
                    .collect()
            };

            for sign in signs {
                send_packet(framed, sign).await;
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_landing(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    chat_tx: &Arc<Sender<String>>,
    sound_tx: &Arc<Sender<SoundEffect>>,
    x: f64,
    y: f64,
    z: f64,
    on_ground: bool,
) {
    if on_ground && !state.was_on_ground {
        let damage_eligible = (state.gamemode == GameMode::Survival
            || state.gamemode == GameMode::Adventure)
            && !state.is_flying;

        if damage_eligible && state.fall_distance > 3.0 {
            let damage = (state.fall_distance - 3.0).round();
            tracing::info!(
                "[FALL] landing: fall_distance={}, was_on_ground={}, y={}, gamemode={:?}, first_pos={}",
                state.fall_distance,
                state.was_on_ground,
                y,
                state.gamemode,
                state.first_position_received
            );
            let died = state.damage_player(framed, damage, player_registry).await;
            let sound = if state.fall_distance > 7.0 {
                "game.player.hurt.fall.big"
            } else {
                "game.player.hurt.fall.small"
            };
            sound_tx.send((sound, x, y, z, 1.0, 63)).ok();
            if died {
                chat_tx
                    .send(ChatBuilder::plain_json(&format!(
                        "{} hit the ground too hard",
                        state.name
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
    world: &Arc<World>,
    tile_entities: &Arc<RwLock<TileEntityStore>>,
    server_loaded_chunks: &Arc<RwLock<HashSet<(i32, i32)>>>,
    new_chunk_x: i32,
    new_chunk_z: i32,
    view_distance: i32,
    loaded_chunks: &mut std::collections::HashSet<(i32, i32)>,
) {
    send_chunks(
        framed,
        client_protocol,
        world,
        tile_entities,
        server_loaded_chunks,
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
        if item_id == EMPTY_HAND {
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

#[cfg(test)]
mod xp_drop_tests {
    use super::level_to_xp_drop;

    #[test]
    fn it_is_seven_per_level() {
        assert_eq!(level_to_xp_drop(1), 7);
        assert_eq!(level_to_xp_drop(2), 14);
        assert_eq!(level_to_xp_drop(5), 35);
        assert_eq!(level_to_xp_drop(14), 98);
    }

    #[test]
    fn a_player_with_no_levels_drops_nothing() {
        assert_eq!(level_to_xp_drop(0), 0);
    }

    #[test]
    fn it_is_capped_at_a_hundred() {
        assert_eq!(level_to_xp_drop(15), 100);
        assert_eq!(level_to_xp_drop(100), 100);
        assert_eq!(level_to_xp_drop(i32::MAX), 100);
    }

    #[test]
    fn a_negative_level_drops_nothing() {
        assert_eq!(level_to_xp_drop(-1), 0);
        assert_eq!(level_to_xp_drop(i32::MIN), 0);
    }

    #[test]
    fn the_drop_rises_with_the_level_until_the_cap() {
        let mut last = 0;
        for level in 0..40 {
            let drop = level_to_xp_drop(level);
            assert!(drop >= last, "level {level} dropped less than {level} - 1");
            assert!((0..=100).contains(&drop), "level {level} gave {drop}");
            last = drop;
        }
    }
}

#[cfg(test)]
mod keep_alive_tests {
    use std::time::{Duration, Instant};

    use super::{KEEP_ALIVE_INTERVAL, KeepAliveAction, keep_alive_action};

    #[test]
    fn a_fresh_connection_is_probed() {
        assert_eq!(keep_alive_action(None), KeepAliveAction::Probe);
    }

    #[test]
    fn an_unanswered_probe_is_a_timeout() {
        let sent = Some((7, Instant::now()));
        assert_eq!(keep_alive_action(sent), KeepAliveAction::Timeout);
    }

    #[test]
    fn it_does_not_matter_how_long_ago_the_probe_went_out() {
        let just_now = Some((1, Instant::now()));
        let a_while = Some((1, Instant::now() - Duration::from_secs(600)));
        assert_eq!(keep_alive_action(just_now), KeepAliveAction::Timeout);
        assert_eq!(keep_alive_action(a_while), KeepAliveAction::Timeout);
    }

    #[test]
    fn answering_returns_to_probing() {
        let mut outstanding = Some((3, Instant::now()));
        assert_eq!(keep_alive_action(outstanding), KeepAliveAction::Timeout);

        outstanding = None;
        assert_eq!(keep_alive_action(outstanding), KeepAliveAction::Probe);
    }

    #[test]
    fn the_window_is_between_one_and_two_intervals() {
        assert_eq!(KEEP_ALIVE_INTERVAL, Duration::from_secs(15));
        let worst_case = KEEP_ALIVE_INTERVAL * 2;
        assert!(
            worst_case <= Duration::from_secs(30),
            "a client would get longer than vanilla's 30s"
        );
    }
}
