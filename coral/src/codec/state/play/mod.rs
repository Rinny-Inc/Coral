use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, atomic::Ordering::Relaxed},
    time::{Duration, Instant},
};

use coral_command::{CommandContext, CommandResult};
use coral_protocol::packets::play::{
    ClientSettings, NamedSoundEffect, PlayerAbilities, PluginMessage, ResourcePackResult,
    ResourcePackStatus,
    block::{
        BlockBreakAnimation, BlockChange, DigStatus, HeldItemChange, ItemEntityMetadata,
        PlayerBlockPlacement, PlayerDig,
    },
    chat::{
        ChatMessage, ChatMessageOut, TabComplete, TabCompleteResponse,
        builder::{ChatAppender, ChatBuilder, ChatColor},
    },
    entity::{
        ArmAnimation, CollectItem, DestroyEntities, EntityAction, EntityActionType,
        EntityAnimation, EntityAnimationType, EntityEquipment, EntityHeadLook, EntityLook,
        EntityLookAndMove, EntityMetadata, EntityRelativeMove, EntityTeleport, EntityVelocity,
        SpawnExperienceOrb, SpawnObject, SpawnPlayer, UseBed, UseEntity, UseEntityAction,
    },
    game::{
        ChangeGameState, ClientStatus, ClientStatusAction, EntityStatus, EntityStatusType,
        GameStateChangeReason, Respawn, SetExperience, UpdateHealth,
    },
    inventory::{
        ClickWindow, CloseWindow, ConfirmTransaction, CreativeInventoryAction, Inventory,
        ItemStack, SetSlot, WindowItems,
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
    experience::{self, XpOrb, xp_needed_for_level},
    items::{
        armor::{apply_armor_reduction, total_defense},
        drops::block_drop,
        potions::PotionEffect,
    },
    mining::break_time_ticks,
    player::Player,
    player::registry::{PlayerRegistry, next_entity_id},
    projectile::{Projectile, ProjectileKind},
};
use coral_types::{GameMode, ToolMaterial, dist_xz, dist3, look_direction};
use coral_world::{
    blocks::{Block, WorldBlocks, placement_metadata},
    chunk::{ChunkData, UnloadChunk},
    generator::FlatWorldGenerator,
    playerdata::{PlayerData, save_player_data},
    time::TimeUpdate,
    weather::WeatherState,
};
use tokio::{
    net::TcpStream,
    sync::{RwLock, broadcast::Sender},
    time::interval,
};
use tokio_stream::StreamExt;
use tokio_util::codec::Framed;

use crate::{
    EquipmentUpdate, ServerContext, SoundEffect,
    codec::{Codec, PlayerState, WindowType, is_normal_disconnect, kick, send_packet},
};

mod interact;
mod ticking;

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
        world_blocks,
        generator,
        spawn_point,
        world_dir,
        xp_orbs,
        world_time,
        chest_storage,
        fluid_queue,
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

    let mut keep_alive_interval = interval(Duration::from_secs(15)); // 30 seconds is timed out

    loop {
        tokio::select! {
            _ = keep_alive_interval.tick() => {
                state.keep_alive_count += 1;
                state.last_sent_keep_alive = Some((state.keep_alive_count, std::time::Instant::now()));
                send_packet(framed, KeepAlive { id: state.keep_alive_count }).await;
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
                    &channels,
                ).await;
            }
            Ok((eid, x, y, z)) = bed_rx.recv() => {
                if eid == state.entity_id {
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
            Ok((sender_eid, id, x, y, z, ox, oy, oz, data, count)) = particle_rx.recv() => {
                if state.entity_id == sender_eid {
                    continue;
                }
                // FIXME: cause crash
                /*send_packet(framed, WorldParticles {
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
            Ok((from, to, message)) = private_msg_rx.recv() => {
                if state.name.as_deref() != Some(&to) {
                    continue;
                }

                state.last_message_from = Some(from.clone());

                let json = ChatAppender::new()
                    .add(ChatBuilder::new(format!("{} -> You: ", from))
                        .color(ChatColor::Gray).italic())
                    .add(ChatBuilder::new(&message).color(ChatColor::Gray).italic())
                    .build();
                send_packet(framed, ChatMessageOut::from_json(&json)).await;
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
            Ok((sound, x, y, z, volume, pitch)) = sound_rx.recv() => {
                // TODO: filter sounds that are sent client side & skip sound maker
                send_packet(framed, NamedSoundEffect {
                    sound, x, y, z, volume, pitch
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
                world_time.store(time_of_day, Relaxed);
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

            Ok(mv) = pos_rx.recv() => {
                if mv.entity_id == state.entity_id {
                    continue;
                }
                let visible = if let Some(me) = player_registry.get(&state.uuid.unwrap_or_default()).await {
                    entity_tracker.read().await.is_visible_to(mv.entity_id, me.x, me.z)
                } else {
                    false
                };

                if !visible {
                    continue;
                }

                // particles
                /*let was_on_ground = player_registry.get(&uuid).await
                    .map(|p| p.on_ground)
                    .unwrap_or(true);

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
                if Some(target_uuid) != state.uuid {
                    continue;
                }

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

                let new_cx = (x as i32) >> 4;
                let new_cz = (x as i32) >> 4;
                if new_cx != state.chunk_x || new_cz != state.chunk_z {
                    state.chunk_x = new_cx;
                    state.chunk_z = new_cz;
                    update_chunks(framed, client_protocol, &world_blocks, &generator, new_cx, new_cz, config.server.view_distance, &mut state.loaded_chunks).await;
                }

                state.was_on_ground = false;
            }
            Ok((target_uuid, reason)) = kick_rq_rx.recv() => {
                if Some(target_uuid) != state.uuid {
                    continue;
                }
                kick(framed, &format!("§c{}", reason)).await;
                break;
            }

            Ok((uuid, amount)) = xp_pickup_rx.recv() => {
                if Some(uuid) != state.uuid {
                    continue;
                }
                state.xp_total += amount;
                let (level, progress) = experience::xp_to_level(state.xp_total);
                state.xp_level = level;
                state.xp_progress = progress;

                send_packet(framed, SetExperience {
                    experience_bar: progress,
                    level,
                    total_experience: state.xp_total
                }).await;

                channels.sound_tx.send(("random.orb".to_string(), 0.0, 0.0, 0.0, 0.5, 63)).ok();
            }
            Ok((eid, x, y, z, amount)) = xp_orb_spawn_rx.recv() => {
                send_packet(framed, SpawnExperienceOrb {
                    entity_id: eid,
                    x, y, z,
                    count: amount as i16
                }).await;
            }
            Ok((eid, x, y, z)) = xp_orb_move_rx.recv() => {
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
            Ok((eid, entity_flags, skin_parts)) = meta_rx.recv() => {
                if eid == state.entity_id {
                    continue;
                }
                send_packet(framed, EntityMetadata {
                    entity_id: eid,
                    entity_flags,
                    skin_parts
                }).await;
            }
            Ok((x, y, z, block_id, metadata)) = block_rx.recv() => {
                send_packet(framed, BlockChange {
                    x, y, z,
                    block_id,
                    block_metadata: metadata
                }).await;
            }
            Ok((eid, x, y, z, stage)) = break_rx.recv() => {
                if eid == state.entity_id {
                    continue;
                }
                send_packet(framed, BlockBreakAnimation {
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
                            username: player.username.clone(),
                            properties: player.properties.clone(),
                            gamemode: player.gamemode as i32,
                            ping: player.latency_ms
                        }).await;
                    } else {
                        send_packet(framed, PlayerListItem17 {
                            username: player.username.clone(),
                            online: true,
                            ping: player.latency_ms as i16
                        }).await;
                    }
                    if let Some(me) = player_registry.get(&state.uuid.unwrap_or_default()).await
                        && dist_xz(player.x, player.z, me.x, me.z) > config.tracking.player
                    {
                        continue;
                    }
                    send_spawn_player(framed, &player).await;
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
                    && let Some(uuid_val) = state.uuid
                    && let Some(me) = player_registry.get(&uuid_val).await
                    && let Some(attacker) = player_registry.get_by_entity_id(attacker_eid).await
                {
                    let magnitude = dist_xz(me.x, me.z, attacker.x, attacker.z).max(0.0001);
                    let mut vx = ((me.x - attacker.x) / magnitude) * 0.4;
                    let mut vz = ((me.z - attacker.z) / magnitude) * 0.4;

                    if attacker.is_sprinting {
                        let yaw_rad = (attacker.yaw * std::f32::consts::PI / 180.0) as f64;
                        vx += -yaw_rad.sin() * 0.4;
                        vz +=  yaw_rad.cos() * 0.4;
                    }

                    send_packet(framed, EntityVelocity {
                        entity_id: state.entity_id,
                        vx,
                        vy: 0.4,
                        vz,
                    }).await;
                }
            }
            Ok((eid, x, y, z, item_id, count, metadata)) = item_rx.recv() => {
                let visible = if let Some(me) = player_registry.get(&state.uuid.unwrap_or_default()).await {
                    entity_tracker.read().await.is_visible_to(eid, me.x, me.z)
                } else {
                    false
                };

                if !visible {
                    continue;
                }
                send_packet(framed, SpawnObject {
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
            Ok((_uuid, gamemode)) = gm_rx.recv() => {
                if state.uuid.is_some() {
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
                                let (actual, last) = state.latency_ms;
                                state.latency_ms.1 = actual;
                                state.latency_ms.0 = sent_time.elapsed().as_millis() as i32;
                                if let Some(uuid) = state.uuid {
                                    player_registry.update_latency(uuid, actual).await;
                                    if ping_to_bar(actual) == ping_to_bar(last) {
                                        continue;
                                    }
                                    channels.ping_tx.send((uuid, actual)).ok();
                                }
                            }
                            continue;
                        }

                        if let Some(held) = packet.as_any().downcast_ref::<HeldItemChange>() {
                            let slot = held.slot.clamp(0, 8) as u8;
                            state.held_slot = slot;

                            let internal_idx = Inventory::packet_to_internal(36 + slot as i16)
                                .unwrap_or(slot as usize);

                            state.held_item = state.inventory.slots[internal_idx]
                                .as_ref()
                                .map(|s| s.item_id)
                                .unwrap_or(-1);
                            if let Some(uuid) = state.uuid {
                                player_registry.update_held_slot(uuid, slot).await;
                                player_registry.update_held_item(uuid, state.held_item).await;
                            }
                            send_held_equip(&channels.equip_tx, state);
                            state.eating = None;
                            state.bow_charging = None;
                            state.try_retract_fishing_hook(&projectiles, &channels.despawn_tx).await;
                            continue;
                        }

                        if let Some(dig) = packet.as_any().downcast_ref::<PlayerDig>() {
                            match dig.status {
                                DigStatus::StartDig => {
                                    let block = world_blocks.get(dig.x, dig.y, dig.z, &generator).await;

                                    if state.gamemode == GameMode::Creative {
                                        world_blocks.set(dig.x, dig.y, dig.z, Block::air(), &generator).await;
                                        channels.block_tx.send((dig.x, dig.y as i32, dig.z, 0, 0)).ok();
                                        channels.sound_tx.send((
                                            block_break_sound(block.id).to_string(),
                                            dig.x as f64 + 0.5, dig.y as f64 + 0.5, dig.z as f64 + 0.5,
                                            1.0, 63
                                        )).ok();
                                        continue;
                                    }
                                    let required = break_time_ticks(
                                        &item_registry,
                                        &block_registry,
                                        state.held_item,
                                        block.id,
                                        false,
                                        state.was_on_ground,
                                    );

                                    state.breaking_block = Some((dig.x, dig.y as i32, dig.z));
                                    state.breaking_started_tick = state.tick_count;
                                    state.breaking_required_ticks = required;
                                    channels.break_tx.send((state.entity_id, dig.x, dig.y as i32, dig.z, 0)).ok();
                                }
                                DigStatus::CancelDig => {
                                    if let Some((bx, by, bz)) = state.breaking_block.take() {
                                        channels.break_tx.send((state.entity_id, bx, by, bz, 255)).ok();
                                    }
                                }
                                DigStatus::FinishDig => {
                                    if state.gamemode != GameMode::Survival {
                                        continue;
                                    }
                                    if let Some((bx, by, bz)) = state.breaking_block.take() {
                                        let block = world_blocks.get(bx, by as u8, bz, &generator).await;

                                        let required_ticks = break_time_ticks(
                                            &item_registry,
                                            &block_registry,
                                            state.held_item,
                                            block.id,
                                            false,
                                            state.was_on_ground,
                                        );
                                        let elapsed = (state.tick_count - state.breaking_started_tick).max(0) as u32;

                                        if elapsed < required_ticks {
                                            channels.block_tx.send((bx, by, bz, block.id as i32, block.metadata)).ok();
                                            state.breaking_block = Some((bx, by, bz));
                                            continue;
                                        }
                                        world_blocks.set(bx, by as u8, bz, Block::air(), &generator).await;
                                        channels.block_tx.send((
                                            bx, by, bz,
                                            0, 0
                                        )).ok();
                                        channels.break_tx.send((state.entity_id, bx, by, bz, 10)).ok();
                                        channels.particle_tx.send((
                                            state.entity_id,
                                            37,
                                            bx as f32 + 0.5,
                                            by as f32 + 0.5,
                                            bz as f32 + 0.5,
                                            0.3, 0.3, 0.3,
                                            block.id as f32,
                                            8
                                        )).ok();
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

                                            let xp = experience::xp_for_block(block.id);
                                            if xp > 0 {
                                                let orb_eid = next_entity_id();
                                                let ox = bx as f64 + 0.5;
                                                let oy = by as f64 + 0.5;
                                                let oz = bz as f64 + 0.5;

                                                xp_orbs.write().await.push(XpOrb {
                                                    entity_id: orb_eid,
                                                    x: ox, y: oy, z: oz,
                                                    vy: 0.2,
                                                    amount: xp,
                                                    ticks_alive: 0
                                                });
                                                channels.xp_orb_spawn_tx.send((orb_eid, ox, oy, oz, xp)).ok();
                                            }
                                        }
                                    }
                                    let broke = state.damage_item(1, &item_registry);
                                    sync_held_slot(framed, state, &player_registry, &channels.equip_tx, broke).await;
                                }
                                DigStatus::DropItem(is_itemstack) => {
                                    let hotbar_slot = state.held_slot as usize;

                                    if state.inventory.slots[hotbar_slot].is_none() {
                                        continue;
                                    }

                                    let item = if is_itemstack {
                                        state.inventory.slots[hotbar_slot].take()
                                    } else {
                                        if let Some(slot) = state.inventory.slots[hotbar_slot].as_mut() {
                                            slot.count -= 1;
                                            let dropped = ItemStack {
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

                                        send_packet(framed, SetSlot {
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
                                        send_held_equip(&channels.equip_tx, state);

                                        if let Some(p) = player_registry.get_by_entity_id(state.entity_id).await {
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

                                        send_packet(framed, HeldItemChange {
                                            slot: state.held_slot as i16
                                        }).await;
                                    }
                                }
                                DigStatus::ShootOrFinishEating => {
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
                                            let (dx, dy, dz) = look_direction(p.yaw, p.pitch);
                                            let speed = power as f64 * 3.0;
                                            let arrow_eid = next_entity_id();

                                            let proj = Projectile {
                                                entity_id: arrow_eid,
                                                owner_entity_id: state.entity_id,
                                                kind: ProjectileKind::Arrow,
                                                x: p.x, y: p.y + 1.5, z: p.z,
                                                vx: dx * speed,
                                                vy: dy * speed,
                                                vz: dz * speed,
                                                ticks_alive: 0,
                                                left_owner: false,
                                            };

                                            projectiles.write().await.push(proj.clone());
                                            channels.projectile_spawn_tx.send((
                                                arrow_eid, state.entity_id, ProjectileKind::Arrow,
                                                proj.x, proj.y, proj.z,
                                                proj.vx, proj.vy, proj.vz
                                            )).ok();

                                            channels.sound_tx.send(("random.bow".to_string(), p.x, p.y, p.z, 1.0, 63)).ok();

                                            if state.gamemode == GameMode::Survival {
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
                                            let (dx, dy, dz) = look_direction(p.yaw, p.pitch);
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
                                                ticks_alive: 0,
                                                left_owner: false,
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
                                            send_packet(framed, SetSlot {
                                                window_id: 0, slot: (36 + hotbar) as i16,
                                                item_id: -1, count: 0, metadata: 0
                                            }).await;
                                            state.held_item = -1;
                                            if let Some(uuid) = state.uuid {
                                                player_registry.update_held_item(uuid, -1).await;
                                            }
                                            send_held_equip(&channels.equip_tx, state);
                                        }
                                    }
                                }
                            }
                            continue;
                        }

                        if let Some(place) = packet.as_any().downcast_ref::<PlayerBlockPlacement>() {
                            let Some(face) = place.face.clone() else {
                                if interact::try_with_item(state, &item_registry, &player_registry, &projectiles, &channels).await {
                                    continue;
                                }
                                continue;
                            };
                            println!("[PLACE 1] held={} face={:?} pos=({},{},{})",
                                    place.held_item_id, face, place.x, place.y, place.z);
                            if interact::try_with_block(framed, place, state, &player_registry, &world_blocks, &world_time, &generator, &chest_storage, &fluid_queue, &channels).await
                                || place.held_item_id == -1
                                || state.gamemode >= GameMode::Adventure
                            {
                                continue;
                            }
                            println!("[PLACE 2] held={} face={:?} pos=({},{},{})",
                                    place.held_item_id, face, place.x, place.y, place.z);
                            let (tx, ty, tz) = face.to_placement(place.x, place.y as i32, place.z);

                            if !(0..=255).contains(&ty) {
                                continue;
                            }

                            let block_id = place.held_item_id as i32;
                            if block_id <= 0 || block_id > 255 {
                                continue;
                            }

                            let block_meta = {
                                let item_meta = state.inventory.slots[state.held_slot as usize]
                                    .as_ref()
                                    .map(|s| s.metadata as u8)
                                    .unwrap_or(0);

                                let yaw = player_registry.get(&state.uuid.unwrap_or_default())
                                    .await
                                    .map(|p| p.yaw)
                                    .unwrap_or(0.0);

                                let cursor_y = place.cursor_y as f32 / 16.0; // 0-15 -> 0.0-1.0

                                placement_metadata(block_id as u8, item_meta, face, yaw, cursor_y)
                            };

                            if state.gamemode == GameMode::Survival {
                                let hotbar_slot = state.held_slot as usize;
                                let Some(slot) = state.inventory.slots[hotbar_slot].as_mut() else {
                                    continue;
                                };
                                slot.count -= 1;
                                let remaining_count = slot.count;
                                let item_id = slot.item_id;
                                let metadata = slot.metadata;

                                if remaining_count == 0 {
                                    state.inventory.slots[hotbar_slot] = None;
                                }

                                let packed_slot = (36 + hotbar_slot) as i16;
                                send_packet(framed, SetSlot {
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
                                send_held_equip(&channels.equip_tx, state);
                            }

                            world_blocks.set(tx, ty as u8, tz, Block::new(block_id as u8, block_meta), &generator).await;
                            channels.block_tx.send((tx, ty, tz, block_id, block_meta)).ok();
                            channels.sound_tx.send((
                                block_break_sound(block_id as u8).to_string(),
                                tx as f64 + 0.5, ty as f64 + 0.5, tz as f64 + 0.5,
                                1.0, 63
                            )).ok();
                            continue;
                        }

                        if packet.as_any().downcast_ref::<ArmAnimation>().is_some() {
                            channels.anim_tx.send((state.entity_id, EntityAnimationType::SwingArm)).ok();
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
                                    args,
                                    reply_target: state.last_message_from.clone(),
                                    is_op: state.is_op
                                };

                                match dispatcher.dispatch(ctx).await {
                                    CommandResult::Success(msg) => {
                                        send_packet(framed, ChatMessageOut::from_json(&msg)).await;
                                    }
                                    CommandResult::Error(msg) => {
                                        send_packet(framed, ChatBuilder::new(&msg).color(ChatColor::Red).into_packet()).await;
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

                            send_packet(framed, TabCompleteResponse {
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
                            let yaw = normalize_yaw(yaw);
                            let pitch = pitch.clamp(-90.0, 90.0);

                            let dx = ((x - p.x) * 32.0).round() as i64;
                            let dy = ((y - p.y) * 32.0).round() as i64;
                            let dz = ((z - p.z) * 32.0).round() as i64;

                            let position_changed = dx != 0 || dy != 0 || dz != 0;

                            if mv.position.is_some() {
                                if !mv.on_ground && y < p.y {
                                    state.fall_distance += (p.y - y) as f32;
                                } else if y > p.y {
                                    state.fall_distance = 0.0;
                                }

                                if position_changed {
                                    entity_tracker.write().await.update_position(
                                        state.entity_id,
                                        x, y, z
                                    );
                                    let new_chunk_x = (x as i32) >> 4;
                                    let new_chunk_z = (z as i32) >> 4;

                                    if new_chunk_x != state.chunk_x || new_chunk_z != state.chunk_z {
                                        if !state.first_position_received {
                                            state.first_position_received = true;
                                            player_registry.update_position(&uuid, x, y, z, yaw, pitch, mv.on_ground).await;
                                            channels.pos_tx.send(MovementBroadcast {
                                                uuid,
                                                entity_id: state.entity_id,
                                                kind: MoveKind::Teleport {
                                                    x, y, z, yaw, pitch,
                                                    on_ground: mv.on_ground
                                                },
                                                head_yaw: Some(yaw)
                                            }).ok();
                                            handle_landing(framed, state, &player_registry, &channels.chat_tx, &channels.sound_tx, x, y, z, mv.on_ground).await;
                                            continue;
                                        }
                                        state.chunk_x = new_chunk_x;
                                        state.chunk_z = new_chunk_z;
                                        update_chunks(framed, client_protocol, &world_blocks, &generator, new_chunk_x, new_chunk_z, config.server.view_distance, &mut state.loaded_chunks).await;
                                    }

                                    let distance = ((x - p.x).powi(2) + (z - p.z).powi(2)).sqrt();
                                    if state.is_sprinting {
                                        state.food_exhaustion += 0.1 * distance as f32;
                                    } else {
                                        state.food_exhaustion += 0.01 * distance as f32;
                                    }
                                }
                            }

                            let rotation_changed = (yaw - p.yaw).abs() > 0.5 || (pitch - p.pitch).abs() > 0.5;
                            let needs_teleport = dx.abs() > 127 || dy.abs() > 127 || dz.abs() > 127;

                            if position_changed || rotation_changed {
                                handle_landing(framed, state, &player_registry, &channels.chat_tx, &channels.sound_tx, x, y, z, mv.on_ground).await;
                                player_registry.update_position(&uuid, x, y, z, yaw, pitch, mv.on_ground).await;
                            }

                            let kind = if needs_teleport {
                                MoveKind::Teleport {
                                    x, y, z, yaw, pitch, on_ground: mv.on_ground,
                                }
                            } else if position_changed && rotation_changed {
                                MoveKind::LookAndRelative {
                                    dx: dx as i8, dy: dy as i8, dz: dz as i8,
                                    yaw, pitch, on_ground: mv.on_ground,
                                }
                            } else if position_changed {
                                MoveKind::Relative {
                                    dx: dx as i8, dy: dy as i8, dz: dz as i8,
                                    on_ground: mv.on_ground,
                                }
                            } else if rotation_changed {
                                MoveKind::Look {
                                    yaw, pitch,
                                    on_ground: mv.on_ground,
                                }
                            } else {
                                continue;
                            };

                            channels.pos_tx.send(MovementBroadcast {
                                uuid,
                                entity_id: state.entity_id,
                                kind,
                                head_yaw: if rotation_changed {
                                    Some(yaw)
                                } else {
                                    None
                                }
                            }).ok();
                            continue;
                        }

                        if packet.as_any().downcast_ref::<CloseWindow>().is_some() {
                            if let Some(cursor) = state.cursor_item.take() {
                                let leftover = insert_into_inventory(&mut state.inventory, cursor);
                                if let Some(dropped) = leftover {
                                    // TODO
                                    // drop in world if inventory full - spawn item entoty at player pos
                                    // reuse existing drop logic
                                }
                            }
                            state.open_window = None;
                            continue;
                        }

                        if packet.as_any().downcast_ref::<ConfirmTransaction>().is_some() {
                            // client acknowledged our confirmation
                            continue;
                        }

                        if let Some(status) = packet.as_any().downcast_ref::<ResourcePackStatus>() {
                            match status.result {
                                ResourcePackResult::Loaded | ResourcePackResult::Accepted => {}
                                ResourcePackResult::Decline => {
                                    if !config.resource_pack.forced {
                                        continue;
                                    }
                                    kick(framed, "You must accept the ressource pack to play!").await;
                                    break;
                                }
                                ResourcePackResult::Failed => {
                                    if !config.resource_pack.forced {
                                        continue;
                                    }
                                    kick(framed, "Resource pack download failed!").await;
                                    break;
                                }
                            }
                            continue;
                        }

                        if let Some(creative) = packet.as_any().downcast_ref::<CreativeInventoryAction>() {
                            if state.gamemode != GameMode::Creative {
                                continue;
                            }

                            let internal = Inventory::packet_to_internal(creative.slot);
                            if let Some(idx) = internal {
                                if creative.item_id == -1 {
                                    state.inventory.slots[idx] = None;
                                } else {
                                    state.inventory.slots[idx] = Some(ItemStack {
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
                                    send_held_equip(&channels.equip_tx, state);
                                }

                                if (5..=8).contains(&idx) {
                                    state.sync_armor(&player_registry, &channels.equip_tx).await;
                                }
                            }
                            continue;
                        }

                        if let Some(click) = packet.as_any().downcast_ref::<ClickWindow>() {
                            send_packet(framed, ConfirmTransaction {
                                window_id: click.window_id,
                                action_number: click.action_number,
                                accepted: true
                            }).await;

                            if let Some(open) = state.open_window.clone()
                                && click.window_id == open.window_id()
                            {
                                match open {
                                    WindowType::Chest { window_id, pos } => {
                                        handle_chest_click(
                                            framed,
                                            state,
                                            pos,
                                            window_id,
                                            click,
                                            &chest_storage,
                                        ).await;
                                    }
                                    _ => {}
                                }
                                continue;
                            }

                            if let Some(idx) = Inventory::packet_to_internal(click.slot)
                                && (5..=8).contains(&idx)
                            {
                                state.sync_armor(&player_registry, &channels.equip_tx).await;
                            }
                            continue;
                        }

                        if let Some(status) = packet.as_any().downcast_ref::<ClientStatus>() {
                            if let ClientStatusAction::PerformRespawn = status.action
                                && state.is_dead
                            {
                                state.health = 20.0;
                                state.food = 20;
                                state.food_saturation = 5.0;
                                state.is_dead = false;

                                if let Some(uuid) = state.uuid {
                                    player_registry.update_health(uuid, state.health, state.food, state.food_saturation).await;
                                }

                                send_packet(framed, Respawn {
                                    dimension: 0,
                                    difficulty: config.world.difficulty,
                                    gamemode: u8::from(state.gamemode),
                                    level_type: "flat".to_string()
                                }).await;

                                let (sx, sy, sz, syaw, spitch) = if let Some((bx, by, bz)) = state.bed_spawn {
                                    (bx as f64 + 0.5, by as f64 + 1.0, bz as f64 + 0.5, 0.0, 0.0)
                                } else {
                                    *spawn_point.read().await
                                };

                                if let Some(uuid) = state.uuid {
                                    player_registry.update_position(&uuid, sx, sy, sz, syaw, spitch, false).await;
                                }

                                let spawn_cx = (sx as i32) >> 4;
                                let spawn_cz = (sz as i32) >> 4;

                                state.loaded_chunks.clear();
                                send_chunks(
                                    framed,
                                    client_protocol,
                                    &world_blocks,
                                    &generator,
                                    spawn_cx, spawn_cz,
                                    config.server.view_distance,
                                    &mut state.loaded_chunks
                                ).await;

                                state.chunk_x = spawn_cx;
                                state.chunk_z = spawn_cz;

                                send_packet(framed, PlayerPositionAndLook {
                                    x: sx,
                                    y: sy,
                                    z: sz,
                                    yaw: 0.0,
                                    pitch: 0.0,
                                    on_ground: false
                                }).await;

                                send_packet(framed, UpdateHealth {
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
                                    EntityActionType::LeaveBed => {
                                        state.is_sleeping = false;
                                        player_registry.update_sleeping(uuid, false).await;
                                        channels.anim_tx.send((state.entity_id, EntityAnimationType::LeaveBed)).ok();
                                        continue;
                                    }
                                    EntityActionType::StartSneaking => Some((true, true)),
                                    EntityActionType::StopSneaking => Some((true, false)),
                                    EntityActionType::StartSprinting => {
                                        // server authority dont allow sprint if food < 6
                                        if state.food > 6 {
                                            Some((false, true))  // sprinting on
                                        } else {
                                            // deny sprint
                                            send_packet(framed, PlayerAbilities {
                                                flags: 0x00,
                                                fly_speed: 0.05,
                                                walk_speed: 0.1,
                                            }).await;
                                            None
                                        }
                                    }
                                    EntityActionType::StopSprinting => Some((false, false)),
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
                            match use_entity.action {
                                UseEntityAction::Attack => {
                                    if state.gamemode == GameMode::Spectator {
                                        continue;
                                    }
                                    let players = player_registry.get_all().await;
                                    if let Some(target) = players.iter()
                                        .find(|p| p.entity_id == use_entity.target_entity_id)
                                    {
                                        if target.is_dead || target.no_damage_ticks > 0 {
                                            continue;
                                        }
                                        player_registry.update_no_damage_ticks(target.uuid, 10).await;
                                        if let Some(me) = player_registry.get(&state.uuid.unwrap_or_default()).await {
                                            let reach = if state.gamemode == GameMode::Creative {
                                                5.0
                                            } else {
                                                4.0
                                            };
                                            if dist3(me.x, me.y, me.z, target.x, target.y, target.z) > reach {
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

                                                let xp_amount = level_to_xp_drop(1); // TODO: add xp_level, xp_total to coral_server::Player
                                                if xp_amount > 0 {
                                                    let orb_eid = next_entity_id();
                                                    xp_orbs.write().await.push(XpOrb {
                                                        entity_id: orb_eid,
                                                        x: target.x, y: target.y + 0.5, z: target.z,
                                                        vy: 0.3,
                                                        amount: xp_amount,
                                                        ticks_alive: 0,
                                                    });
                                                    channels.xp_orb_spawn_tx.send((orb_eid, target.x, target.y + 0.5, target.z, xp_amount)).ok();
                                                }
                                            }

                                            channels.dmg_tx.send((target.uuid, new_health, target.food, target.food_saturation, state.entity_id)).ok();

                                            let broke = state.damage_item(2, &item_registry);
                                            sync_held_slot(framed, state, &player_registry, &channels.equip_tx, broke).await;

                                            channels.sound_tx.send((
                                                "game.player.hurt".to_string(),
                                                target.x, target.y, target.z,
                                                1.0, 63
                                            )).ok();
                                            channels.status_tx.send((use_entity.target_entity_id, EntityStatusType::HurtAnimation)).ok();
                                            channels.anim_tx.send((use_entity.target_entity_id, EntityAnimationType::TakeDamage)).ok();

                                            if is_critical {
                                                channels.anim_tx.send((state.entity_id, EntityAnimationType::CriticalEffect(false))).ok();
                                                channels.particle_tx.send((state.entity_id, 1, target.x as f32, target.y as f32 + 1.0, target.z as f32, 0.3, 0.3, 0.3, 0.0, 8)).ok();
                                            }
                                        }
                                    }
                                }
                                _ => {}
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
                                send_packet(framed, PluginMessage::brand("Coral")).await;
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
    if let (Some(uuid), Some(name)) = (state.uuid, &state.name) {
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
                gamemode: u8::from(state.gamemode),
                inventory: inventory_data,
                xp_total: state.xp_total,
                bed_spawn: state.bed_spawn,
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

// ;) nPaper
fn ping_to_bar(ping: i32) -> u8 {
    if ping <= 0 {
        return 5;
    }
    if ping < 150 {
        return 0;
    }
    if ping < 300 {
        return 1;
    }
    if ping < 600 {
        return 2;
    }
    if ping < 1000 {
        return 3;
    }
    4
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
    if level == 0 {
        return 0;
    }

    let xp_in_current_level = xp_needed_for_level(level - 1);
    let drop = (xp_in_current_level * 7).min(100);
    drop.max(0)
}

fn send_held_equip(equip_tx: &Arc<Sender<EquipmentUpdate>>, state: &PlayerState) {
    let (item_id, count, metadata) = state.inventory.slots[state.held_slot as usize]
        .as_ref()
        .map(|s| (s.item_id, s.count, s.metadata))
        .unwrap_or((-1, 0, 0));

    equip_tx
        .send((state.entity_id, 0, item_id, count, metadata))
        .ok();
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

async fn sync_held_slot(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    equip_tx: &Arc<Sender<EquipmentUpdate>>,
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
pub async fn send_chunks(
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
            let died = state.damage_player(framed, damage, player_registry).await;
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

async fn handle_chest_click(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    pos: (i32, i32, i32),
    window_id: u8,
    click: &ClickWindow,
    chest_storage: &Arc<RwLock<HashMap<(i32, i32, i32), Vec<Option<ItemStack>>>>>,
) {
    let slot = click.slot;

    // map a window slot to either chest storage or player inventory
    // returns (is_chest, index)
    fn resolve(slot: i16) -> Option<(bool, usize)> {
        match slot {
            0..=26 => Some((true, slot as usize)),              // chest
            27..=53 => Some((false, (slot - 27 + 9) as usize)), // main inv -> internal 9-35
            54..=62 => Some((false, (slot - 54) as usize)),     // hotbar -> internal 0-8
            _ => None,
        }
    }

    // TODO: a function so theres no block and the lock is dropped when needed
    {
        let mut storage = chest_storage.write().await;
        let chest = storage.entry(pos).or_insert_with(|| vec![None; 27]);
        match click.mode {
            0 => {
                let Some((is_chest, idx)) = resolve(slot) else {
                    return;
                };

                let slot_item = if is_chest {
                    chest[idx].take()
                } else {
                    state.inventory.slots[idx].take()
                };

                let cursor = state.cursor_item.take();

                if is_chest {
                    chest[idx] = cursor;
                } else {
                    state.inventory.slots[idx] = cursor;
                }
                state.cursor_item = slot_item;
            }
            1 => {
                let Some((is_chest, idx)) = resolve(slot) else {
                    return;
                };
                let moving = if is_chest {
                    chest[idx].take()
                } else {
                    state.inventory.slots[idx].take()
                };
                if let Some(stack) = moving {
                    if is_chest {
                        let leftover = insert_into_inventory(&mut state.inventory, stack);
                        chest[idx] = leftover;
                    } else {
                        let leftover = insert_into_chest(chest, stack);
                        state.inventory.slots[idx] = leftover;
                    }
                }
            }
            _ => {}
        }
    }

    resend_chest_window(framed, state, pos, window_id, chest_storage).await;
}

fn insert_into_inventory(inv: &mut Inventory, mut stack: ItemStack) -> Option<ItemStack> {
    // first merge into existing matching stacks
    for existing in inv.slots.iter_mut().take(36).flatten() {
        if existing.item_id == stack.item_id
            && existing.metadata == stack.metadata
            && existing.count < 64
        {
            let space = 64 - existing.count;
            let move_n = space.min(stack.count);
            existing.count += move_n;
            stack.count -= move_n;
            if stack.count == 0 {
                return None;
            }
        }
    }
    // second place into empty space
    for slot in inv.slots.iter_mut().take(36) {
        if slot.is_none() {
            *slot = Some(stack);
            return None;
        }
    }
    Some(stack) // didnt fit
}
fn insert_into_chest(chest: &mut [Option<ItemStack>], mut stack: ItemStack) -> Option<ItemStack> {
    for existing in chest.iter_mut().flatten() {
        if existing.item_id == stack.item_id
            && existing.metadata == stack.metadata
            && existing.count < 64
        {
            let space = 64 - existing.count;
            let move_n = space.min(stack.count);
            existing.count += move_n;
            stack.count -= move_n;
            if stack.count == 0 {
                return None;
            }
        }
    }
    for existing in chest.iter_mut() {
        if existing.is_none() {
            *existing = Some(stack);
            return None;
        }
    }
    Some(stack)
}
async fn resend_chest_window(
    framed: &mut Framed<TcpStream, Codec>,
    state: &PlayerState,
    pos: (i32, i32, i32),
    window_id: u8,
    chest_storage: &Arc<RwLock<HashMap<(i32, i32, i32), Vec<Option<ItemStack>>>>>,
) {
    let storage = chest_storage.read().await;
    let empty = vec![None; 27];
    let chest = storage.get(&pos).unwrap_or(&empty);

    let mut slots: Vec<(i16, u8, i16)> = Vec::with_capacity(63);
    for item in chest.iter().take(27) {
        match item {
            Some(s) => slots.push((s.item_id, s.count, s.metadata)),
            None => slots.push((-1, 0, 0)),
        }
    }
    for internal in 9..36 {
        match &state.inventory.slots[internal] {
            Some(s) => slots.push((s.item_id, s.count, s.metadata)),
            None => slots.push((-1, 0, 0)),
        }
    }
    for internal in 0..9 {
        match &state.inventory.slots[internal] {
            Some(s) => slots.push((s.item_id, s.count, s.metadata)),
            None => slots.push((-1, 0, 0)),
        }
    }

    send_packet(framed, WindowItems { window_id, slots }).await;

    send_packet(
        framed,
        SetSlot {
            window_id: -1, // 255
            slot: -1,
            item_id: state.cursor_item.as_ref().map(|s| s.item_id).unwrap_or(-1),
            count: state.cursor_item.as_ref().map(|s| s.count).unwrap_or(0),
            metadata: state.cursor_item.as_ref().map(|s| s.metadata).unwrap_or(0),
        },
    )
    .await;
}
