use std::{
    sync::{Arc, atomic::Ordering::Relaxed},
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
    crafting::RecipeRegistry,
    falling::TNT_FUSE_TICKS,
    items::ItemRegistry,
    player::registry::{PlayerRegistry, next_entity_id},
    projectile::{Projectile, ProjectileKind, launch_velocity},
    tile_entities::TileEntityStore,
};
use coral_types::{
    GameMode,
    ids::{blocks, items},
};
use coral_world::{
    blocks::{
        Block,
        fluid::{Fluid, FluidKind, is_replaceable, occludes_ray},
    },
    world::World,
};
use tokio::{net::TcpStream, sync::RwLock};
use tokio_util::codec::Framed;

use crate::{
    Channels,
    codec::{
        Codec, PlayerState, send_packet,
        state::play::{crafting_window::open_crafting_window, window_ops::open_tile_entity_window},
    },
    fluid_sim::{FluidQueue, queue_fluid_update},
};

const RAY_MAX_BLOCKS: usize = 6;

/// Return true if the itme interaction went well
pub async fn try_with_item(
    state: &mut PlayerState,
    item_registry: &Arc<ItemRegistry>,
    player_registry: &Arc<PlayerRegistry>,
    projectiles: &Arc<RwLock<Vec<Projectile>>>,
    channels: &Channels,
) -> bool {
    if let Some((_hunger, _saturation)) = item_registry.food_value(state.held_item.id())
        && state.food < 20
        && state.eating.is_none()
    {
        state.eating = Some(Instant::now());
        channels
            .anim_tx
            .send((state.entity_id, EntityAnimationType::Eat))
            .ok();
    }

    match state.held_item.id() {
        items::BOW => {
            state.bow_charging = Some(Instant::now());
            true
        }
        items::FISHING_ROD => {
            if !state
                .try_retract_fishing_hook(projectiles, &channels.despawn_tx)
                .await
                && let Some(p) = player_registry.get(&state.uuid).await
            {
                let speed = 1.5;
                let (vx, vy, vz) = launch_velocity(p.get_head_direction(), p.velocity, speed);
                let hook_eid = next_entity_id();

                let (eye_x, eye_y, eye_z) = p.get_head_position();
                let proj = Projectile {
                    entity_id: hook_eid,
                    owner_entity_id: state.entity_id,
                    kind: ProjectileKind::FishingHook,
                    x: eye_x,
                    y: eye_y,
                    z: eye_z,
                    vx,
                    vy,
                    vz,
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
        items::POTION => {
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
    item_registry: &Arc<ItemRegistry>,
    world: &Arc<World>,
    tile_entities: &Arc<RwLock<TileEntityStore>>,
    fluid_queue: &Arc<RwLock<FluidQueue>>,
    channels: &Channels,
    clicked: Option<(i32, i32, i32)>,
) -> bool {
    match place.held_item_id {
        items::FLINT_AND_STEEL => {
            let Some((bx, by, bz)) = clicked else {
                return false;
            };
            if world.get(bx, by as u8, bz).await.id != blocks::TNT {
                // TODO: place fire and remove durability to the flint and steel
                return false;
            }
            world.set(bx, by as u8, bz, Block::air()).await;
            channels.block_tx.send((bx, by, bz, 0, 0)).ok();
            channels
                .tnt_prime_tx
                .send((bx as f64 + 0.5, by as f64, bz as f64 + 0.5, TNT_FUSE_TICKS))
                .ok();
            return true;
        }
        items::BUCKET => {
            tracing::debug!(
                "[BUCKET] entered branch, checking ({}, {}, {})",
                place.x,
                place.y,
                place.z
            );

            let Some(p) = player_registry.get(&state.uuid).await else {
                return false;
            };

            let hit = match clicked {
                Some(l) => Some((l.0, l.1, l.2)),
                None => {
                    let (x, y, z) = p.get_head_position();
                    raytrace_for_fluid(x, y, z, p.yaw, p.pitch, RAY_MAX_BLOCKS, world).await
                }
            };

            tracing::debug!("[BUCKET] raytrace hit = {:?}", hit);

            let Some(hit) = hit else {
                return false;
            };

            let (bx, by, bz) = hit;
            if !(0..=255).contains(&by) {
                return false;
            }

            let target = world.get(bx, by as u8, bz).await;

            tracing::debug!("[BUCKET] target id={} meta={}", target.id, target.metadata);

            let Some(fluid) = Fluid::from_block_id(target.id) else {
                tracing::debug!("[BUCKET] not a fluid block, aborting");
                return false;
            };
            if !Fluid::is_source(target.id, target.metadata) {
                tracing::debug!("[BUCKET] fluid found but not a source (falling/edge), aborting");
                return false;
            }

            tracing::debug!("[BUCKET] confirmed source, removing");
            // remove the source
            world.set(bx, by as u8, bz, Block::air()).await;
            channels.block_tx.send((bx, by, bz, 0, 0)).ok();

            // recalculate neighbor fluids (it may now drain)
            queue_fluid_update(bx, by, bz, fluid_queue).await;
            tracing::debug!(
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
                            nbt: None,
                        },
                        player_registry,
                        item_registry,
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
                    sound,
                    bx as f64 + 0.5,
                    by as f64 + 0.5,
                    bz as f64 + 0.5,
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
            let (block_id, meta, tx, ty, tz) = match *face as u8 {
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
                    let meta = match *face as u8 {
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

            world
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
                    .consume_held_one(framed, player_registry, item_registry, channels)
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

        let existing = world.get(tx, ty as u8, tz).await;
        // only place into air or replaceable blocks
        if !existing.is_air() && !is_replaceable(existing.id) {
            return false;
        }

        let source_id = fluid.block_id();
        world.set(tx, ty as u8, tz, Block::new(source_id, 0)).await;
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
                        item_id: items::BUCKET,
                        count: 1,
                        metadata: 0,
                        durability: 0,
                        nbt: None,
                    },
                    player_registry,
                    item_registry,
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
            .send((sound, tx as f64, ty as f64, tz as f64, 1.0, 63))
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
    item_registry: &Arc<ItemRegistry>,
    world: &Arc<World>,
    tile_entities: &Arc<RwLock<TileEntityStore>>,
    fluid_queue: &Arc<RwLock<FluidQueue>>,
    channels: &Channels,
    recipes: &Arc<RecipeRegistry>,
) -> bool {
    if place.face.is_none() {
        return false;
    };
    let clicked = world.get(place.x, place.y, place.z).await;

    if try_with_item_on_block(
        framed,
        place,
        state,
        player_registry,
        item_registry,
        world,
        tile_entities,
        fluid_queue,
        channels,
        Some((place.x, place.y as i32, place.z)),
    )
    .await
    {
        return true;
    }

    match clicked.id {
        blocks::FURNACE | blocks::LIT_FURNACE => {
            return open_tile_entity_window(
                framed,
                state,
                (place.x, place.y as i32, place.z),
                clicked.id,
                tile_entities,
                &channels.chest_anim_tx,
                &channels.sound_tx,
            )
            .await;
        }
        blocks::CRAFTING_TABLE => {
            if !(state.is_sneaking && place.held_item_id > 0) {
                return open_crafting_window(framed, state, recipes).await;
            }
            false
        }
        blocks::CHEST | blocks::TRAPPED_CHEST => {
            if !(state.is_sneaking && place.held_item_id > 0) {
                return open_tile_entity_window(
                    framed,
                    state,
                    (place.x, place.y as i32, place.z),
                    clicked.id,
                    tile_entities,
                    &channels.chest_anim_tx,
                    &channels.sound_tx,
                )
                .await;
            }
            false
        }
        blocks::BED => {
            if !is_night(world.time.load(Relaxed)) {
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

fn voxel_path(
    eye: (f64, f64, f64),
    direction: (f64, f64, f64),
    max_blocks: usize,
) -> Vec<(i32, i32, i32)> {
    let (ox, oy, oz) = eye;
    let (dx, dy, dz) = direction;

    let mut block = (ox.floor() as i32, oy.floor() as i32, oz.floor() as i32);

    if dx == 0.0 && dy == 0.0 && dz == 0.0 {
        return if max_blocks == 0 {
            Vec::new()
        } else {
            vec![block]
        };
    }

    let step = (
        if dx > 0.0 { 1 } else { -1 },
        if dy > 0.0 { 1 } else { -1 },
        if dz > 0.0 { 1 } else { -1 },
    );

    let first_boundary = |origin: f64, dir: f64, start: i32| -> f64 {
        if dir > 0.0 {
            (start as f64 + 1.0 - origin) / dir
        } else if dir < 0.0 {
            (start as f64 - origin) / dir
        } else {
            f64::INFINITY
        }
    };

    let mut t_next = (
        first_boundary(ox, dx, block.0),
        first_boundary(oy, dy, block.1),
        first_boundary(oz, dz, block.2),
    );
    let t_delta = (
        if dx == 0.0 {
            f64::INFINITY
        } else {
            (1.0 / dx).abs()
        },
        if dy == 0.0 {
            f64::INFINITY
        } else {
            (1.0 / dy).abs()
        },
        if dz == 0.0 {
            f64::INFINITY
        } else {
            (1.0 / dz).abs()
        },
    );

    let mut path = Vec::with_capacity(max_blocks);
    for _ in 0..max_blocks {
        path.push(block);

        if t_next.0 <= t_next.1 && t_next.0 <= t_next.2 {
            block.0 += step.0;
            t_next.0 += t_delta.0;
        } else if t_next.1 <= t_next.2 {
            block.1 += step.1;
            t_next.1 += t_delta.1;
        } else {
            block.2 += step.2;
            t_next.2 += t_delta.2;
        }
    }
    path
}

fn look_direction(yaw: f32, pitch: f32) -> (f64, f64, f64) {
    let yaw_rad = (yaw as f64).to_radians();
    let pitch_rad = (pitch as f64).to_radians();

    (
        -yaw_rad.sin() * pitch_rad.cos(),
        -pitch_rad.sin(),
        yaw_rad.cos() * pitch_rad.cos(),
    )
}

async fn raytrace_for_fluid(
    eye_x: f64,
    eye_y: f64,
    eye_z: f64,
    yaw: f32,
    pitch: f32,
    max_blocks: usize,
    world: &Arc<World>,
) -> Option<(i32, i32, i32)> {
    let direction = look_direction(yaw, pitch);

    for (bx, by, bz) in voxel_path((eye_x, eye_y, eye_z), direction, max_blocks) {
        if !(0..=255).contains(&by) {
            continue;
        }
        let block = world.get(bx, by as u8, bz).await;

        if Fluid::is_fluid(block.id) {
            return Fluid::is_source(block.id, block.metadata).then_some((bx, by, bz));
        }
        if occludes_ray(block.id) {
            return None;
        }
    }

    None
}

#[cfg(test)]
mod raytrace_for_fluid_tests {
    use super::{RAY_MAX_BLOCKS, raytrace_for_fluid};
    use coral_types::ids::blocks::WATER as WATER_SOURCE;
    use coral_world::{blocks::Block, generator::VoidWorldGenerator, world::World};
    use std::sync::Arc;

    fn void_world() -> Arc<World> {
        Arc::new(World::new(Arc::new(VoidWorldGenerator::new())))
    }

    #[tokio::test]
    async fn the_trace_starts_from_the_eye_position_not_the_origin() {
        let world = void_world();
        world.set(300, 62, -140, Block::new(WATER_SOURCE, 0)).await;

        let hit = raytrace_for_fluid(300.5, 65.62, -139.5, 0.0, 90.0, RAY_MAX_BLOCKS, &world).await;
        assert_eq!(hit, Some((300, 62, -140)));

        let as_direction =
            raytrace_for_fluid(0.0, -1.0, 0.0, 0.0, 90.0, RAY_MAX_BLOCKS, &world).await;
        assert_eq!(as_direction, None, "the old call could never have hit");
    }

    #[tokio::test]
    async fn the_hit_is_the_fluid_block_itself() {
        let world = void_world();
        world.set(0, 60, 0, Block::new(WATER_SOURCE, 0)).await;

        let hit = raytrace_for_fluid(0.5, 65.62, 0.5, 0.0, 90.0, RAY_MAX_BLOCKS, &world)
            .await
            .expect("the trace should reach the water");
        let block = world.get(hit.0, hit.1 as u8, hit.2).await;
        assert_eq!(block.id, WATER_SOURCE, "hit a block that is not the fluid");
    }

    #[tokio::test]
    async fn a_solid_block_stops_the_trace() {
        let world = void_world();
        world.set(0, 62, 0, Block::new(WATER_SOURCE, 0)).await;
        world.set(0, 63, 0, Block::new(1, 0)).await; // stone lid

        let hit = raytrace_for_fluid(0.5, 65.62, 0.5, 0.0, 90.0, RAY_MAX_BLOCKS, &world).await;
        assert_eq!(hit, None);
    }

    #[tokio::test]
    async fn flowing_water_is_not_a_source() {
        let world = void_world();
        world.set(0, 62, 0, Block::new(WATER_SOURCE, 3)).await;

        let hit = raytrace_for_fluid(0.5, 65.62, 0.5, 0.0, 90.0, RAY_MAX_BLOCKS, &world).await;
        assert_eq!(hit, None);
    }

    #[tokio::test]
    async fn nothing_in_range_is_not_a_hit() {
        let world = void_world();
        world.set(0, 20, 0, Block::new(WATER_SOURCE, 0)).await;

        let hit = raytrace_for_fluid(0.5, 65.62, 0.5, 0.0, 90.0, RAY_MAX_BLOCKS, &world).await;
        assert_eq!(hit, None, "the water is well past the 6-block reach");
    }
}

#[cfg(test)]
mod voxel_path_tests {

    use super::{RAY_MAX_BLOCKS, look_direction, voxel_path};

    #[track_caller]
    fn assert_contiguous(path: &[(i32, i32, i32)]) {
        for pair in path.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let delta = ((b.0 - a.0).abs(), (b.1 - a.1).abs(), (b.2 - a.2).abs());
            let moved = delta.0 + delta.1 + delta.2;
            assert_eq!(
                moved, 1,
                "stepped from {a:?} to {b:?}, which is not one block on one axis"
            );
        }
    }

    #[test]
    fn the_walk_starts_in_the_block_holding_the_eye() {
        let path = voxel_path((10.7, 65.2, -3.4), (0.0, -1.0, 0.0), 4);
        assert_eq!(path[0], (10, 65, -4), "floor, not truncate, for -3.4");
    }

    #[test]
    fn a_downward_ray_descends_one_block_at_a_time() {
        let path = voxel_path((0.5, 65.62, 0.5), (0.0, -1.0, 0.0), 6);
        assert_eq!(
            path,
            vec![
                (0, 65, 0),
                (0, 64, 0),
                (0, 63, 0),
                (0, 62, 0),
                (0, 61, 0),
                (0, 60, 0),
            ]
        );
    }

    #[test]
    fn the_walk_is_capped_at_the_block_budget_in_every_direction() {
        for (yaw, pitch) in [
            (0.0f32, 0.0f32),
            (45.0, 0.0),
            (45.0, 45.0),
            (123.0, -37.0),
            (-90.0, 89.0),
            (17.0, -89.0),
        ] {
            let direction = look_direction(yaw, pitch);
            let path = voxel_path((0.5, 64.5, 0.5), direction, RAY_MAX_BLOCKS);

            assert_eq!(
                path.len(),
                RAY_MAX_BLOCKS,
                "yaw {yaw} pitch {pitch} walked {} blocks",
                path.len()
            );
            assert_contiguous(&path);
        }
    }

    #[test]
    fn no_block_is_visited_twice() {
        for yaw in (0..360).step_by(7) {
            for pitch in (-80..=80).step_by(13) {
                let direction = look_direction(yaw as f32, pitch as f32);
                let path = voxel_path((0.5, 64.5, 0.5), direction, 24);

                let mut seen = path.clone();
                seen.sort_unstable();
                seen.dedup();
                assert_eq!(
                    seen.len(),
                    path.len(),
                    "yaw {yaw} pitch {pitch} revisited a block"
                );
                assert_contiguous(&path);
            }
        }
    }

    #[test]
    fn the_walk_sees_everything_a_dense_sample_sees() {
        for yaw in (0..360).step_by(11) {
            for pitch in (-75..=75).step_by(15) {
                let direction = look_direction(yaw as f32, pitch as f32);
                let eye = (0.5, 64.5, 0.5);
                let path = voxel_path(eye, direction, 12);

                let mut sampled: Vec<(i32, i32, i32)> = Vec::new();
                let mut t = 0.0f64;
                while t <= 12.0 {
                    let block = (
                        (eye.0 + direction.0 * t).floor() as i32,
                        (eye.1 + direction.1 * t).floor() as i32,
                        (eye.2 + direction.2 * t).floor() as i32,
                    );
                    if sampled.last() != Some(&block) {
                        sampled.push(block);
                    }
                    t += 0.0005;
                }

                let mut cursor = 0usize;
                for block in &sampled {
                    let found = path[cursor..].iter().position(|p| p == block);
                    match found {
                        Some(offset) => cursor += offset + 1,
                        None => {
                            assert!(
                                !path.contains(block),
                                "yaw {yaw} pitch {pitch}: {block:?} is in the \
                                 walk but out of order"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_zero_direction_ray_yields_only_its_own_block() {
        let path = voxel_path((3.5, 64.5, -7.5), (0.0, 0.0, 0.0), 6);
        assert_eq!(path, vec![(3, 64, -8)]);
    }

    #[test]
    fn a_zero_budget_walks_nothing() {
        assert!(voxel_path((0.5, 64.5, 0.5), (0.0, -1.0, 0.0), 0).is_empty());
    }

    #[test]
    fn an_axis_with_no_component_never_moves() {
        let path = voxel_path((0.5, 64.5, 0.5), look_direction(0.0, 0.0), 8);
        for block in &path {
            assert_eq!((block.0, block.1), (0, 64), "{block:?} drifted");
        }
        assert_eq!(path.len(), 8);
    }
}

#[cfg(test)]
mod ray_budget_tests {
    use std::sync::Arc;

    use coral_world::blocks::Block;
    use coral_world::generator::VoidWorldGenerator;
    use coral_world::world::World;

    use super::{RAY_MAX_BLOCKS, raytrace_for_fluid};
    use coral_types::ids::blocks::{STONE, WATER as WATER_SOURCE};

    fn void_world() -> Arc<World> {
        Arc::new(World::new(Arc::new(VoidWorldGenerator::new())))
    }

    #[tokio::test]
    async fn a_source_at_the_last_block_of_the_budget_is_found() {
        let world = void_world();
        world.set(0, 59, 0, Block::new(WATER_SOURCE, 0)).await;

        let hit = raytrace_for_fluid(0.5, 64.5, 0.5, 0.0, 90.0, RAY_MAX_BLOCKS, &world).await;
        assert_eq!(hit, Some((0, 59, 0)));
    }

    #[tokio::test]
    async fn a_source_past_the_budget_is_not_found() {
        let world = void_world();
        world.set(0, 58, 0, Block::new(WATER_SOURCE, 0)).await;

        let hit = raytrace_for_fluid(0.5, 64.5, 0.5, 0.0, 90.0, RAY_MAX_BLOCKS, &world).await;
        assert_eq!(hit, None, "the seventh block is out of reach");
    }

    #[tokio::test]
    async fn the_budget_is_six_blocks_on_a_diagonal_as_well() {
        let world = void_world();
        world.set(0, 64, 5, Block::new(WATER_SOURCE, 0)).await;
        assert_eq!(
            raytrace_for_fluid(0.5, 64.5, 0.5, 0.0, 0.0, RAY_MAX_BLOCKS, &world).await,
            Some((0, 64, 5))
        );

        let further = void_world();
        further.set(0, 64, 6, Block::new(WATER_SOURCE, 0)).await;
        assert_eq!(
            raytrace_for_fluid(0.5, 64.5, 0.5, 0.0, 0.0, RAY_MAX_BLOCKS, &further).await,
            None
        );
    }

    #[tokio::test]
    async fn a_wall_hides_the_water_behind_it() {
        let world = void_world();
        world.set(0, 63, 0, Block::new(STONE, 0)).await;
        world.set(0, 62, 0, Block::new(WATER_SOURCE, 0)).await;

        let hit = raytrace_for_fluid(0.5, 64.5, 0.5, 0.0, 90.0, RAY_MAX_BLOCKS, &world).await;
        assert_eq!(hit, None, "a bucket cannot reach through stone");
    }

    #[tokio::test]
    async fn passable_blocks_do_not_hide_the_water() {
        for passable in [
            coral_types::ids::blocks::TORCH,
            coral_types::ids::blocks::TALLGRASS,
            coral_types::ids::blocks::WHEAT,
            coral_types::ids::blocks::RAIL,
        ] {
            let world = void_world();
            world.set(0, 63, 0, Block::new(passable, 0)).await;
            world.set(0, 62, 0, Block::new(WATER_SOURCE, 0)).await;

            let hit = raytrace_for_fluid(0.5, 64.5, 0.5, 0.0, 90.0, RAY_MAX_BLOCKS, &world).await;
            assert_eq!(hit, Some((0, 62, 0)), "block {passable} should not occlude");
        }
    }

    #[tokio::test]
    async fn flowing_water_in_the_way_stops_the_ray() {
        let world = void_world();
        world.set(0, 63, 0, Block::new(WATER_SOURCE, 3)).await;
        world.set(0, 62, 0, Block::new(WATER_SOURCE, 0)).await;

        let hit = raytrace_for_fluid(0.5, 64.5, 0.5, 0.0, 90.0, RAY_MAX_BLOCKS, &world).await;
        assert_eq!(hit, None, "the flowing water ends the trace");
    }

    #[tokio::test]
    async fn the_block_holding_the_eye_counts() {
        let world = void_world();
        world.set(0, 64, 0, Block::new(WATER_SOURCE, 0)).await;

        let hit = raytrace_for_fluid(0.5, 64.5, 0.5, 0.0, 0.0, RAY_MAX_BLOCKS, &world).await;
        assert_eq!(hit, Some((0, 64, 0)));
    }
}
