use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering::Relaxed},
    },
    time::Instant,
};

use coral_protocol::packets::play::{
    block::{BlockFace, PlayerBlockPlacement},
    chat::{
        ChatMessageOut,
        builder::{ChatBuilder, ChatColor},
    },
    entity::{EntityAnimationType, TileEntity, UseBed},
    inventory::{ItemStack, SignEditorOpen},
};
use coral_server::{
    items::ItemRegistry,
    player::registry::{PlayerRegistry, next_entity_id},
    projectile::{Projectile, ProjectileKind},
};
use coral_types::GameMode;
use coral_world::{
    blocks::{
        Block, WorldBlocks,
        fluid::{Fluid, FluidKind, is_replaceable},
    },
    generator::FlatWorldGenerator,
};
use tokio::{net::TcpStream, sync::RwLock};
use tokio_util::codec::Framed;

use crate::{
    Channels,
    codec::{Codec, PlayerState, send_packet, state::play::window_ops::open_tile_entity_window},
    fluid_sim::queue_fluid_update,
};

/// Return true if the itme interaction went well
pub async fn try_with_item(
    state: &mut PlayerState,
    item_registry: &Arc<ItemRegistry>,
    player_registry: &Arc<PlayerRegistry>,
    projectiles: &Arc<RwLock<Vec<Projectile>>>,
    channels: &Channels,
) -> bool {
    if let Some((_hunger, _saturation)) = item_registry.food_value(state.held_item)
        && state.food < 20
        && state.eating.is_none()
    {
        state.eating = Some(Instant::now());
        channels
            .anim_tx
            .send((state.entity_id, EntityAnimationType::Eat))
            .ok();
    }

    match state.held_item {
        261 => {
            state.bow_charging = Some(Instant::now());
            true
        }
        346 => {
            if !state
                .try_retract_fishing_hook(projectiles, &channels.despawn_tx)
                .await
                && let Some(p) = player_registry.get(&state.uuid).await
            {
                let (dx, dy, dz) = p.get_head_direction();
                let speed = 1.5;
                let hook_eid = next_entity_id();

                let (eye_x, eye_y, eye_z) = p.get_head_position();
                let proj = Projectile {
                    entity_id: hook_eid,
                    owner_entity_id: state.entity_id,
                    kind: ProjectileKind::FishingHook,
                    x: eye_x,
                    y: eye_y,
                    z: eye_z,
                    vx: dx + p.velocity.0 * speed,
                    vy: dy + p.velocity.1 * speed,
                    vz: dz + p.velocity.2 * speed,
                    ticks_alive: 0,
                    left_owner: false,
                };
                projectiles.write().await.push(proj.clone());
                channels
                    .projectile_spawn_tx
                    .send((
                        hook_eid,
                        state.entity_id,
                        ProjectileKind::FishingHook,
                        proj.x,
                        proj.y,
                        proj.z,
                        proj.vx,
                        proj.vy,
                        proj.vz,
                    ))
                    .ok();

                state.fishing_hook_eid = Some(hook_eid);
            }
            channels
                .anim_tx
                .send((state.entity_id, EntityAnimationType::SwingArm))
                .ok();
            true
        }
        373 => {
            let meta = state.inventory.slots[state.held_slot as usize]
                .as_ref()
                .map(|s| s.metadata)
                .unwrap_or(0);
            let is_splash = (meta & 0x4000) != 0;

            if !is_splash {
                if state.eating.is_none() {
                    state.eating = Some(Instant::now());
                    channels
                        .anim_tx
                        .send((state.entity_id, EntityAnimationType::Eat))
                        .ok();
                }
                return true;
            }
            false
        }
        _ => false,
    }
}

/// Return true if the item on block interaction went well
pub async fn try_with_item_on_block(
    framed: &mut Framed<TcpStream, Codec>,
    place: &PlayerBlockPlacement,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    world_blocks: &Arc<WorldBlocks>,
    tile_entities: &Arc<RwLock<HashMap<(i32, i32, i32), TileEntity>>>,
    generator: &Arc<FlatWorldGenerator>,
    fluid_queue: &Arc<RwLock<VecDeque<(i32, i32, i32)>>>,
    channels: &Channels,
    clicked: Option<(i32, i32, i32)>,
) -> bool {
    match place.held_item_id {
        325 => {
            println!(
                "[BUCKET] entered branch, checking ({}, {}, {})",
                place.x, place.y, place.z
            );

            let Some(p) = player_registry.get(&state.uuid).await else {
                return false;
            };

            // FIXME: might be problematic
            let hit = match clicked {
                Some(l) => Some((l.0, l.1, l.2)),
                None => {
                    let (x, y, z) = p.get_head_direction();
                    raytrace_for_fluid(x, y, z, p.yaw, p.pitch, 6.0, world_blocks, generator).await
                }
            };

            println!("[BUCKET] raytrace hit = {:?}", hit);

            let Some(hit) = hit else {
                return false;
            };

            let y = hit.1 + 1;

            let target = world_blocks.get(hit.0, y as u8, hit.2, generator).await;
            println!("[BUCKET] target id={} meta={}", target.id, target.metadata);

            let Some(fluid) = Fluid::from_block_id(target.id) else {
                println!("[BUCKET] not a fluid block, aborting");
                return false;
            };
            if !Fluid::is_source(target.id, target.metadata) {
                println!("[BUCKET] fluid found but not a source (falling/edge), aborting");
                return false;
            }

            println!("[BUCKET] confirmed source, removing");
            // remove the source
            world_blocks
                .set(place.x, y as u8, place.z, Block::air())
                .await;
            channels.block_tx.send((place.x, y, place.z, 0, 0)).ok();

            // recalculate neighbor fluids (it may now drain)
            queue_fluid_update(place.x, y, place.z, fluid_queue).await;
            println!(
                "[BUCKET] queued, queue len = {}",
                fluid_queue.read().await.len()
            );

            // repalce empty bucket with filled one (survival only)
            if state.gamemode == GameMode::Survival {
                let filled = fluid.bucket_item();
                state
                    .replace_held_item(
                        framed,
                        ItemStack {
                            item_id: filled,
                            count: 1,
                            metadata: 0,
                            durability: 0,
                        },
                        player_registry,
                        channels,
                    )
                    .await;
            }

            let sound = match fluid.kind {
                FluidKind::Water => "liquid.water",
                FluidKind::Lava => "liquid.lava",
            };
            channels
                .sound_tx
                .send((
                    sound.to_string(),
                    place.x as f64 + 0.5,
                    y as f64 + 0.5,
                    place.z as f64 + 0.5,
                    1.0,
                    63,
                ))
                .ok();
            return true;
        }
        323 => {
            let Some(face) = &place.face else {
                return false;
            };
            let (block_id, meta, tx, ty, tz) = match face.clone() as u8 {
                1 => {
                    let yaw = player_registry
                        .get(&state.uuid)
                        .await
                        .map(|p| p.yaw)
                        .unwrap_or(0.0);
                    let rotation = (((yaw + 180.0) / 22.5).round() as i32 & 0xF) as u8;
                    (63u8, rotation, place.x, place.y as i32 + 1, place.z)
                }
                2..=5 => {
                    let meta = match face.clone() as u8 {
                        2 => 2,
                        3 => 3,
                        4 => 4,
                        5 => 5,
                        _ => 2,
                    };
                    let face = BlockFace::try_from(meta).unwrap();
                    let (tx, ty, tz) = face.to_placement(place.x, place.y as i32, place.z);
                    (68u8, meta, tx, ty, tz)
                }
                _ => return false,
            };

            world_blocks
                .set(tx, ty as u8, tz, Block::new(block_id, meta))
                .await;
            channels
                .block_tx
                .send((tx, ty, tz, block_id as i32, meta))
                .ok();

            tile_entities.write().await.insert(
                (tx, ty, tz),
                TileEntity::Sign {
                    lines: [
                        String::with_capacity(16),
                        String::with_capacity(16),
                        String::with_capacity(16),
                        String::with_capacity(16),
                    ],
                },
            );

            if state.gamemode == GameMode::Survival {
                state
                    .consume_held_one(framed, player_registry, channels)
                    .await;
            }

            state.editing_sign = Some((tx, ty, tz));
            send_packet(
                framed,
                SignEditorOpen {
                    x: tx,
                    y: ty,
                    z: tz,
                },
            )
            .await;
            return true;
        }
        _ => {}
    }

    let Some(face) = &place.face else {
        return false;
    };

    if let Some(fluid) = Fluid::from_bucket_item(place.held_item_id) {
        let (tx, ty, tz) = face.to_placement(place.x, place.y as i32, place.z);

        if !(0..=255).contains(&ty) {
            return false;
        }

        let existing = world_blocks.get(tx, ty as u8, tz, generator).await;
        // only place into air or replaceable blocks
        if !existing.is_air() && !is_replaceable(existing.id) {
            return false;
        }

        let source_id = fluid.block_id();
        world_blocks
            .set(tx, ty as u8, tz, Block::new(source_id, 0))
            .await;
        channels
            .block_tx
            .send((tx, ty, tz, source_id as i32, 0))
            .ok();

        // start flow simulation
        queue_fluid_update(tx, ty, tz, fluid_queue).await;

        // empty the bucket only in survival mode
        if state.gamemode == GameMode::Survival {
            state
                .replace_held_item(
                    framed,
                    ItemStack {
                        item_id: 325,
                        count: 1,
                        metadata: 0,
                        durability: 0,
                    },
                    player_registry,
                    channels,
                )
                .await;
        }

        let sound = match fluid.kind {
            FluidKind::Water => "liquid.water",
            FluidKind::Lava => "liquid.lava",
        };

        channels
            .sound_tx
            .send((sound.to_string(), tx as f64, ty as f64, tz as f64, 1.0, 63))
            .ok();
        return true;
    }
    false
}

/// Return true if the block interaction went well
#[allow(clippy::too_many_arguments)]
pub async fn try_with_block(
    framed: &mut Framed<TcpStream, Codec>,
    place: &PlayerBlockPlacement,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    world_blocks: &Arc<WorldBlocks>,
    world_time: &Arc<AtomicI64>,
    generator: &Arc<FlatWorldGenerator>,
    tile_entities: &Arc<RwLock<HashMap<(i32, i32, i32), TileEntity>>>,
    fluid_queue: &Arc<RwLock<VecDeque<(i32, i32, i32)>>>,
    channels: &Channels,
) -> bool {
    if place.face.is_none() {
        return false;
    };
    let clicked = world_blocks.get(place.x, place.y, place.z, generator).await;

    if try_with_item_on_block(
        framed,
        place,
        state,
        player_registry,
        world_blocks,
        tile_entities,
        generator,
        fluid_queue,
        channels,
        Some((place.x, place.y as i32, place.z)),
    )
    .await
    {
        return true;
    }

    match clicked.id {
        61 | 62 => {
            return open_tile_entity_window(
                framed,
                state,
                (place.x, place.y as i32, place.z),
                tile_entities,
                &channels.chest_anim_tx,
                &channels.sound_tx,
            )
            .await;
        }
        54 | 146 => {
            if !(state.is_sneaking && place.held_item_id > 0) {
                return open_tile_entity_window(
                    framed,
                    state,
                    (place.x, place.y as i32, place.z),
                    tile_entities,
                    &channels.chest_anim_tx,
                    &channels.sound_tx,
                )
                .await;
            }
            false
        }
        26 => {
            if !is_night(world_time.load(Relaxed)) {
                send_packet(
                    framed,
                    ChatMessageOut::from_json(&ChatBuilder::plain_json(
                        "You can only sleep at night",
                    )),
                )
                .await;
                return false;
            }
            state.bed_spawn = Some((place.x, place.y as i32, place.z));
            send_packet(
                framed,
                ChatBuilder::new("Respawn point set")
                    .color(ChatColor::Gray)
                    .into_packet(),
            )
            .await;

            state.is_sleeping = true;
            player_registry.update_sleeping(&state.uuid, true).await;

            send_packet(
                framed,
                UseBed {
                    entity_id: state.entity_id,
                    x: place.x,
                    y: place.y as i32,
                    z: place.z,
                },
            )
            .await;
            channels
                .bed_tx
                .send((state.entity_id, place.x, place.y as i32, place.z))
                .ok();
            true
        }
        _ => false,
    }
}

fn is_night(time_of_day: i64) -> bool {
    (12542..=23459).contains(&time_of_day)
}

async fn raytrace_for_fluid(
    eye_x: f64,
    eye_y: f64,
    eye_z: f64,
    yaw: f32,
    pitch: f32,
    max_dist: f64,
    world_blocks: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
) -> Option<(i32, i32, i32)> {
    let yaw_rad = (yaw as f64).to_radians();
    let pitch_rad = (pitch as f64).to_radians();

    let dx = -yaw_rad.sin() * pitch_rad.cos();
    let dy = -pitch_rad.sin();
    let dz = yaw_rad.cos() * pitch_rad.cos();

    let step = 0.2;
    let mut t = 0.0;

    let mut last_block: Option<(i32, i32, i32)> = None;

    while t <= max_dist {
        let px = eye_x + dx * t;
        let py = eye_y + dy * t;
        let pz = eye_z + dz * t;

        let bx = px.floor() as i32;
        let by = py.floor() as i32;
        let bz = pz.floor() as i32;

        if last_block != Some((bx, by, bz)) {
            last_block = Some((bx, by, bz));

            if (0..=255).contains(&by) {
                let block = world_blocks.get(bx, by as u8, bz, generator).await;

                if Fluid::is_fluid(block.id) && Fluid::is_source(block.id, block.metadata) {
                    return Some((bx, by, bz));
                }
                if !block.is_air() && !is_replaceable(block.id) {
                    return None;
                }
            }
        }

        t += step;
    }

    None
}
