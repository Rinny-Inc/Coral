use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use coral::updater::RestartPlan;
use coral_command::{
    CommandContext, CommandDispatcher, CommandResult, list::usage::ResourceMonitor,
};
use coral_protocol::packets::play::{
    chat::builder::{ChatBuilder, ChatColor},
    entity::TileEntity,
    inventory::ItemStack,
};
use coral_server::{
    bounding_box::EntityBounds,
    entity_tracker::{EntityTracker, TrackedEntity},
    experience::XpOrb,
    explosion::{
        blast_reach, destroyed_blocks, drop_chance, entity_effect, entity_radius,
        explosion_resistance, exposure,
    },
    falling::{
        FallingEntity, FallingKind, MAX_FALL_TICKS, TNT_POWER, bounce as falling_bounce,
        can_fall_through, chained_fuse, should_start_falling, spawn_object_data, spawn_object_type,
        step as falling_step, tnt_launch_velocity,
    },
    items::drops::{DroppedItems, block_drop},
    player::registry::{PlayerRegistry, next_entity_id},
    projectile::{self, Projectile, ProjectileKind},
    smelting::{fuel_burn_ticks, fuel_residue, smelt_result},
    tile_entities::TileEntityStore,
};
use coral_types::{
    DespawnEntity, ExplosionEvent, GameMode, dist_sq3, dist3, ext::TicksExt, ids::blocks,
};
use coral_world::{
    anvil::tile_entity_to_nbt,
    blocks::{Block, fluid::occludes_ray, registry::BlockRegistry},
    nbt::NbtTag,
    weather::{Weather, WeatherState},
    world::World,
};
use rand::RngExt;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{RwLock, broadcast::Sender},
    time::interval,
};
use uuid::Uuid;

use crate::{
    Channels,
    fluid_sim::{FluidQueue, queue_fluid_update},
};

const SPLASH_RADIUS: f64 = 4.0;
const ARROW_SCAN_RADIUS: f64 = 2.0;
const ORB_PICKUP_RADIUS: f64 = 1.0;

/// Why the server is stopping
#[derive(PartialEq)]
enum Stop {
    Exit,
    /// Re-exec this binary
    /// The path comes from the updater rather than from `current_exe()` see [`coral::updater::RestartRequest`]
    Restart(PathBuf),
}

pub fn spawn_shutdown_task(
    shutdown_signal: Arc<Sender<bool>>,
    restart_tx: Arc<Sender<PathBuf>>,
    chat_tx: Arc<Sender<String>>,
    restart_grace: Duration,
    restart_plan: RestartPlan,
    player_registry: Arc<PlayerRegistry>,
    world: Arc<World>,
    world_dir: PathBuf,
    tile_entities: Arc<RwLock<TileEntityStore>>,
) {
    let mut restart_rx = restart_tx.subscribe();
    tokio::spawn(async move {
        drop(restart_tx);
        let stop = tokio::select! {
            _ = tokio::signal::ctrl_c() => Stop::Exit,
            Ok(exe) = restart_rx.recv() => {
                warn_players(&chat_tx, &player_registry, restart_grace).await;
                Stop::Restart(exe)
            }
        };
        tracing::info!("Shutting down, kicking players..");
        shutdown_signal.send(stop != Stop::Exit).ok();

        for _ in 0..50 {
            if player_registry.get_online_count().await == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        tracing::info!("Saving world..");

        let tile_entities_by_chunk: HashMap<(i32, i32), Vec<NbtTag>> = {
            let all_tiles = tile_entities.read().await;
            tile_entity_nbt_by_chunk(&all_tiles)
        };
        world.save(&world_dir, &tile_entities_by_chunk).await;
        match stop {
            Stop::Exit => {
                tracing::info!("World saved. Server closed!");
                std::process::exit(0);
            }
            Stop::Restart(exe) => {
                tracing::info!("World saved. Restarting..");
                restart_into(&restart_plan, &exe);
            }
        }
    });
}

fn tile_entity_nbt_by_chunk(store: &TileEntityStore) -> HashMap<(i32, i32), Vec<NbtTag>> {
    store
        .chunks()
        .map(|(chunk, tiles)| {
            let nbt = tiles
                .iter()
                .filter_map(|((x, y, z), tile)| tile_entity_to_nbt(*x, *y, *z, tile))
                .collect();
            (*chunk, nbt)
        })
        .collect()
}

/// Tell whoever is online that the server is about to go, and give them the
/// grace period to get somewhere safe
/// Skipped entirely on an empty server
async fn warn_players(
    chat_tx: &Arc<Sender<String>>,
    player_registry: &Arc<PlayerRegistry>,
    grace: Duration,
) {
    if player_registry.get_online_count().await == 0 || grace.is_zero() {
        return;
    }

    let secs = grace.as_secs();
    chat_tx
        .send(ChatBuilder::colored_json(
            &format!("Update installed - restarting in {secs}s"),
            ChatColor::Yellow,
        ))
        .ok();

    // a last call
    if grace > Duration::from_secs(6) {
        tokio::time::sleep(grace - Duration::from_secs(5)).await;
        chat_tx
            .send(ChatBuilder::colored_json(
                "Restarting in 5s",
                ChatColor::Red,
            ))
            .ok();
        tokio::time::sleep(Duration::from_secs(5)).await;
    } else {
        tokio::time::sleep(grace).await;
    }
}

fn restart_into(plan: &RestartPlan, exe: &std::path::Path) -> ! {
    let (program, args): (&std::path::Path, Vec<std::ffi::OsString>) = match plan {
        RestartPlan::Command { program, args } => (
            program.as_path(),
            args.iter().map(std::ffi::OsString::from).collect(),
        ),
        RestartPlan::Binary => (exe, std::env::args_os().skip(1).collect()),
    };

    let mut command = std::process::Command::new(program);
    command.args(&args);
    command.env("CORAL_UPDATED_EXE", exe);

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = command.exec();
        tracing::error!("[Updater] Could not exec {}: {err}", program.display());
    }

    #[cfg(windows)]
    {
        match command.spawn() {
            Ok(child) => {
                tracing::info!("[Updater] Restarted as pid {}", child.id());
                std::process::exit(0);
            }
            Err(e) => tracing::error!("[Updater] Could not start {}: {e}", program.display()),
        }
    }

    if matches!(plan, RestartPlan::Command { .. }) {
        tracing::error!("[Updater] Falling back to the updated binary");
        restart_into(&RestartPlan::Binary, exe);
    }

    tracing::error!("[Updater] Start it again manually to pick up the update.");
    std::process::exit(70);
}

pub fn spawn_tick_task(tick_tx: Arc<Sender<()>>, player_registry: Arc<PlayerRegistry>) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_ticks(1));
        loop {
            interval.tick().await;
            player_registry.tick().await;
            tick_tx.send(()).ok();
        }
    });
}
pub fn spawn_world_time_task(
    time_tx: Arc<Sender<(i64, i64)>>,
    player_registry: Arc<PlayerRegistry>,
    wake_tx: Arc<Sender<()>>,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_ticks(1));
        let mut world_age: i64 = 0;
        let mut time_of_day: i64 = 0;

        loop {
            interval.tick().await;
            world_age += 1;

            let online = player_registry.get_online_count().await as usize;
            let sleeping = player_registry.count_sleeping().await;

            if online > 0 && sleeping >= online && (12542..=23459).contains(&time_of_day) {
                time_of_day = 0;
                player_registry.clear_all_sleeping().await;
                wake_tx.send(()).ok();
                time_tx.send((world_age, time_of_day)).ok();
                continue;
            }

            time_of_day = (time_of_day + 1) % 24000;

            if world_age % 20 == 0 {
                time_tx.send((world_age, time_of_day)).ok();
            }
        }
    });
}
pub fn spawn_weather_task(weather_tx: Arc<Sender<WeatherState>>) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_ticks(1));
        let mut weather = Weather::new();

        loop {
            interval.tick().await;
            if let Some(new_state) = weather.tick() {
                weather_tx.send(new_state).ok();
            }
        }
    });
}
pub fn spawn_item_despawn_task(
    despawn_tx: Arc<Sender<DespawnEntity>>,
    item_despawn_secs: u64,
    item_spawn_times: Arc<RwLock<HashMap<i32, Instant>>>,
    item_positions: Arc<RwLock<DroppedItems>>,
    entity_tracker: Arc<RwLock<EntityTracker>>,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            let expired: Vec<i32> = {
                let times = item_spawn_times.read().await;
                times
                    .iter()
                    .filter(|(_, t)| t.elapsed().as_secs() >= item_despawn_secs)
                    .map(|(eid, _)| *eid)
                    .collect()
            };

            if expired.is_empty() {
                continue;
            }
            {
                let mut times = item_spawn_times.write().await;
                let mut positions = item_positions.write().await;

                for eid in &expired {
                    times.remove(eid);
                    positions.remove(*eid);
                }
            }

            entity_tracker.write().await.untrack_many(&expired);
            despawn_tx.send(expired).ok();
        }
    });
}
pub fn spawn_console_task(dispatcher: Arc<CommandDispatcher>, chat_tx: Arc<Sender<String>>) {
    tokio::spawn(async move {
        let stdin = tokio::io::stdin();
        let mut lines = BufReader::new(stdin).lines();

        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }

            let input = if let Some(stripped) = line.strip_prefix('/') {
                stripped.to_string()
            } else {
                line.clone()
            };

            let args: Vec<String> = input.split_whitespace().map(|s| s.to_string()).collect();

            if args.is_empty() {
                continue;
            }

            let ctx = CommandContext {
                sender: "CONSOLE".to_string(),
                args,
                reply_target: None,
                is_op: true,
            };

            match dispatcher.dispatch(ctx).await {
                CommandResult::Success(msg) => tracing::info!("[CONSOLE] {}", msg),
                CommandResult::Error(msg) => tracing::error!("[CONSOLE ERROR] {}", msg),
                CommandResult::Broadcast(msg) => {
                    chat_tx.send(ChatBuilder::plain_json(&msg)).ok();
                }
                CommandResult::None => {}
            }
        }
    });
}
pub fn spawn_world_save_task(
    world: Arc<World>,
    world_dir: PathBuf,
    tile_entities: Arc<RwLock<TileEntityStore>>,
    auto_save_interval: u64,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(auto_save_interval));
        loop {
            interval.tick().await;
            let tile_entities_by_chunk: HashMap<(i32, i32), Vec<NbtTag>> = {
                let all_tiles = tile_entities.read().await;
                tile_entity_nbt_by_chunk(&all_tiles)
            };
            world.save(&world_dir, &tile_entities_by_chunk).await;
            tracing::info!("[World] Auto-Saved.");
        }
    });
}
pub fn spawn_chunk_cache_cleanup_task(world: Arc<World>) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            world
                .blocks
                .evict_stale_chunks(Duration::from_secs(120))
                .await;
        }
    });
}
pub fn spawn_projectile_task(
    projectiles: Arc<RwLock<Vec<Projectile>>>,
    world: Arc<World>,
    player_registry: Arc<PlayerRegistry>,
    channels: Channels,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_ticks(1));
        loop {
            interval.tick().await;

            let positions = player_registry.position_snapshot().await;

            let (to_remove, moves, splash_effects) = {
                let mut list = projectiles.write().await;
                let mut to_remove = vec![];
                let mut moves = vec![];
                let mut splash_effects: Vec<(Uuid, u8, u8, i32)> = vec![];

                for proj in list.iter_mut() {
                    proj.ticks_alive += 1;

                    let gravity = proj.kind.gravity();
                    let drag = 0.99;

                    proj.vy -= gravity;
                    proj.vx *= drag;
                    proj.vy *= drag;
                    proj.vz *= drag;
                    proj.x += proj.vx;
                    proj.y += proj.vy;
                    proj.z += proj.vz;

                    let bx = proj.x.floor() as i32;
                    let by = proj.y.floor() as i32;
                    let bz = proj.z.floor() as i32;

                    let block = world.get(bx, by.clamp(0, 255) as u8, bz).await;
                    let hit_block = !block.is_air() && (0..255).contains(&by);

                    if hit_block {
                        match proj.kind {
                            ProjectileKind::Arrow => {
                                to_remove.push(proj.entity_id);
                            }
                            ProjectileKind::FishingHook => {
                                proj.vx = 0.0;
                                proj.vy = 0.0;
                                proj.vz = 0.0;
                            }
                            ProjectileKind::SplashPotion(meta) => {
                                let effects = coral_server::items::potions::potion_effects(meta);
                                for target in
                                    positions.candidates_near(proj.x, proj.z, SPLASH_RADIUS)
                                {
                                    let dist =
                                        dist3(target.x, target.y, target.z, proj.x, proj.y, proj.z);

                                    if dist <= SPLASH_RADIUS {
                                        let potency = 1.0 - (dist / 4.0);
                                        for pe in &effects {
                                            let scaled_duration =
                                                (pe.duration_ticks as f64 * potency) as i32;
                                            if scaled_duration < 1 {
                                                continue;
                                            }
                                            splash_effects.push((
                                                target.uuid,
                                                pe.kind.clone() as u8,
                                                pe.amplifier,
                                                scaled_duration,
                                            ));
                                        }
                                    }
                                }
                                to_remove.push(proj.entity_id);
                            }
                        }
                        continue;
                    }

                    if proj.kind == ProjectileKind::Arrow {
                        for target in positions.candidates_near(proj.x, proj.z, ARROW_SCAN_RADIUS) {
                            if target.is_dead {
                                continue;
                            }

                            if target.entity_id == proj.owner_entity_id && !proj.left_owner {
                                if dist_sq3(target.x, target.y, target.z, proj.x, proj.y, proj.z)
                                    >= 1.0
                                {
                                    proj.left_owner = true;
                                }
                                continue;
                            }

                            if dist_sq3(target.x, target.y, target.z, proj.x, proj.y, proj.z) < 0.64
                            {
                                let Some(hit) =
                                    player_registry.get_by_entity_id(target.entity_id).await
                                else {
                                    continue;
                                };
                                let speed =
                                    (proj.vx.powi(2) + proj.vy.powi(2) + proj.vz.powi(2)).sqrt();
                                let damage = (speed * 3.0).clamp(2.0, 10.0) as f32;
                                let new_health = (hit.health - damage).max(0.0);

                                player_registry
                                    .update_health(
                                        &target.uuid,
                                        new_health,
                                        hit.food,
                                        hit.food_saturation,
                                    )
                                    .await;
                                channels
                                    .dmg_tx
                                    .send((
                                        target.uuid,
                                        new_health,
                                        hit.food,
                                        hit.food_saturation,
                                        proj.owner_entity_id,
                                    ))
                                    .ok();

                                to_remove.push(proj.entity_id); // TODO: future me -> there's EntityLiving#arrowStuck available on paper
                                break;
                            }
                        }
                    }

                    if proj.ticks_alive > 6000 {
                        to_remove.push(proj.entity_id);
                        continue;
                    }

                    let (yaw, pitch) = projectile::rotation(proj.vx, proj.vy, proj.vz);
                    moves.push((proj.entity_id, proj.x, proj.y, proj.z, yaw, pitch));
                }

                list.retain(|p| !to_remove.contains(&p.entity_id));
                (to_remove, moves, splash_effects)
            };

            for mv in moves {
                channels.projectile_move_tx.send(mv).ok();
            }
            if !to_remove.is_empty() {
                channels.despawn_tx.send(to_remove).ok();
            }
            for (uuid, effect_id, amplifier, duration) in splash_effects {
                channels
                    .splash_effect_tx
                    .send((uuid, effect_id, amplifier, duration))
                    .ok();
            }
        }
    });
}
pub fn spawn_xp_orb_task(
    xp_orbs: Arc<RwLock<Vec<XpOrb>>>,
    world: Arc<World>,
    player_registry: Arc<PlayerRegistry>,
    entity_tracker: Arc<RwLock<EntityTracker>>,
    channels: Channels,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_ticks(1));
        loop {
            interval.tick().await;

            let positions = player_registry.position_snapshot().await;

            let (moves, picked_up_per_player) = {
                let mut orbs = xp_orbs.write().await;
                let mut moves = vec![];
                let mut to_remove = vec![];
                let mut pickups: Vec<(Uuid, i32)> = vec![];

                for orb in orbs.iter_mut() {
                    orb.ticks_alive += 1;
                    orb.vy -= 0.01;
                    orb.vy = orb.vy.clamp(-0.3, 0.3);
                    orb.y += orb.vy;

                    let bx = orb.x.floor() as i32;
                    let by = (orb.y.floor() as i32).clamp(0, 255) as u8;
                    let bz = orb.z.floor() as i32;
                    let below = world.get(bx, by.saturating_sub(1), bz).await;
                    if !below.is_air() && orb.vy < 0.0 {
                        orb.vy = 0.0;
                    }

                    let mut picked = false;
                    for p in positions.candidates_near(orb.x, orb.z, ORB_PICKUP_RADIUS) {
                        if p.is_dead {
                            continue;
                        }
                        if dist3(p.x, p.y, p.z, orb.x, orb.y, orb.z) < ORB_PICKUP_RADIUS {
                            pickups.push((p.uuid, orb.amount));
                            to_remove.push(orb.entity_id);
                            picked = true;
                            break;
                        }
                    }
                    if picked {
                        continue;
                    }

                    if orb.ticks_alive > 6000 {
                        // TODO: make it customable
                        to_remove.push(orb.entity_id);
                        continue;
                    }

                    moves.push((orb.entity_id, orb.x, orb.y, orb.z));
                }
                orbs.retain(|o| !to_remove.contains(&o.entity_id));
                (moves, (to_remove, pickups))
            };

            for mv in moves {
                channels.xp_orb_move_tx.send(mv).ok();
            }
            if !picked_up_per_player.0.is_empty() {
                entity_tracker
                    .write()
                    .await
                    .untrack_many(&picked_up_per_player.0);
                channels.despawn_tx.send(picked_up_per_player.0).ok();
            }
            for (uuid, amount) in picked_up_per_player.1 {
                channels.xp_pickup_tx.send((uuid, amount)).ok();
            }
        }
    });
}
pub fn spawn_resource_monitor_task(monitor: Arc<ResourceMonitor>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(1));
        loop {
            interval.tick().await;
            monitor.sample().await;
        }
    });
}
pub fn spawn_furnace_task(
    tile_entities: Arc<RwLock<TileEntityStore>>,
    world: Arc<World>,
    channels: Channels,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_ticks(1));
        loop {
            interval.tick().await;

            let position: Vec<(i32, i32, i32)> = {
                let storage = tile_entities.read().await;
                storage.furnace_positions().collect()
            };

            if position.is_empty() {
                continue;
            }

            let mut storage = tile_entities.write().await;
            for pos in &position {
                let Some(tile) = storage.get_mut(pos) else {
                    continue;
                };
                let TileEntity::Furnace {
                    input,
                    fuel,
                    output,
                    burn_ticks,
                    burn_ticks_total,
                    cook_ticks,
                    ..
                } = tile
                else {
                    continue;
                };

                let was_burning = *burn_ticks > 0;

                let can_smelt = input
                    .as_ref()
                    .and_then(|i| smelt_result(i.item_id, i.metadata))
                    .map(|(rid, rmeta)| match output {
                        None => true,
                        Some(o) => o.item_id == rid && o.metadata == rmeta && o.count < 64,
                    })
                    .unwrap_or(false);
                if *burn_ticks == 0
                    && can_smelt
                    && let Some(f) = fuel
                    && let Some(burn) = fuel_burn_ticks(f.item_id)
                {
                    *burn_ticks = burn;
                    *burn_ticks_total = burn;
                    let spent = f.item_id;
                    f.count -= 1;
                    if f.count == 0 {
                        *fuel = fuel_residue(spent).map(|id| ItemStack {
                            item_id: id,
                            count: 1,
                            metadata: 0,
                            durability: 0,
                            nbt: None,
                        });
                    }
                }

                let mut changed = false;

                if *burn_ticks > 0 {
                    *burn_ticks -= 1;
                    if can_smelt {
                        *cook_ticks += 1;
                        if *cook_ticks >= 200 {
                            *cook_ticks = 0;
                            let inp = input.as_ref().unwrap();
                            let (rid, rmeta) = smelt_result(inp.item_id, inp.metadata).unwrap();
                            match output {
                                Some(o) => o.count += 1,
                                None => {
                                    *output = Some(ItemStack {
                                        item_id: rid,
                                        count: 1,
                                        metadata: rmeta,
                                        durability: 0,
                                        nbt: None,
                                    })
                                }
                            }
                            let inp = input.as_mut().unwrap();
                            inp.count -= 1;
                            if inp.count == 0 {
                                *input = None;
                            }
                        }
                    } else {
                        *cook_ticks = (*cook_ticks - 2).max(0);
                    }
                    changed = true;
                } else if *cook_ticks > 0 {
                    *cook_ticks = (*cook_ticks - 2).max(0);
                    changed = true;
                }

                let now_burning = *burn_ticks > 0;

                if was_burning != now_burning {
                    let current = world.get(pos.0, pos.1 as u8, pos.2).await;
                    let new_id = if now_burning { 62 } else { 61 };
                    world
                        .set(
                            pos.0,
                            pos.1 as u8,
                            pos.2,
                            Block::new(new_id, current.metadata),
                        )
                        .await;
                    channels
                        .block_tx
                        .send((pos.0, pos.1, pos.2, new_id as i32, current.metadata))
                        .ok();
                }

                if changed {
                    channels.furnace_update_tx.send(*pos).ok();
                }
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_explosion_task(
    world: Arc<World>,
    block_registry: Arc<BlockRegistry>,
    player_registry: Arc<PlayerRegistry>,
    entity_tracker: Arc<RwLock<EntityTracker>>,
    item_spawn_times: Arc<RwLock<HashMap<i32, Instant>>>,
    item_positions: Arc<RwLock<DroppedItems>>,
    fluid_queue: Arc<RwLock<FluidQueue>>,
    tracking_range: f64,
    channels: Channels,
) {
    tokio::spawn(async move {
        let mut requests = channels.explosion_rq_tx.subscribe();
        while let Ok((x, y, z, power)) = requests.recv().await {
            if power <= 0.0 {
                continue;
            }

            let radius = blast_reach(power).max(entity_radius(power).ceil() as i32 + 2);
            let (cx, cy, cz) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
            let mut nearby: HashMap<(i32, i32, i32), u8> = HashMap::new();
            for bx in cx - radius..=cx + radius {
                for by in (cy - radius).max(0)..=(cy + radius).min(255) {
                    for bz in cz - radius..=cz + radius {
                        let block = world.get(bx, by as u8, bz).await;
                        if block.id != blocks::AIR {
                            nearby.insert((bx, by, bz), block.id);
                        }
                    }
                }
            }
            let destroyed = {
                let mut rng = rand::rng();
                let registry = &block_registry;
                let nearby = &nearby;
                destroyed_blocks(x, y, z, power, &mut || rng.random::<f32>(), |bx, by, bz| {
                    nearby
                        .get(&(bx, by, bz))
                        .map(|&id| explosion_resistance(registry.blast_resitance(id)))
                })
            };

            let chance = drop_chance(power);
            let mut cleared = Vec::with_capacity(destroyed.len());
            for &(bx, by, bz) in &destroyed {
                if !(0..=255).contains(&by) {
                    continue;
                }
                let before = world.get(bx, by as u8, bz).await;
                if before.id == blocks::AIR {
                    continue;
                }
                world.set(bx, by as u8, bz, Block::air()).await;
                channels.block_tx.send((bx, by, bz, 0, 0)).ok();
                queue_fluid_update(bx, by, bz, &fluid_queue).await;
                cleared.push((bx, by, bz));

                if before.id == blocks::TNT {
                    let fuse = chained_fuse(rand::rng().random::<f64>());
                    channels
                        .tnt_prime_tx
                        .send((bx as f64 + 0.5, by as f64, bz as f64 + 0.5, fuse))
                        .ok();
                    continue;
                }

                if rand::rng().random::<f32>() < chance
                    && let Some((item_id, count, metadata)) = block_drop(before.id, before.metadata)
                {
                    spawn_blast_drop(
                        (bx as f64 + 0.5, by as f64 + 0.5, bz as f64 + 0.5),
                        (item_id, count, metadata),
                        &entity_tracker,
                        &item_spawn_times,
                        &item_positions,
                        tracking_range,
                        &channels,
                    )
                    .await;
                }
            }

            let solid = |bx: i32, by: i32, bz: i32| {
                nearby
                    .get(&(bx, by, bz))
                    .is_some_and(|&id| occludes_ray(id))
            };
            let mut motion = HashMap::new();
            for player in player_registry.get_all().await {
                if player.is_dead {
                    continue;
                }
                let bound = EntityBounds::player(player.is_sneaking);
                let (dx, dy, dz) = (player.x - x, player.y - y, player.z - z);
                let distance = (dx * dx + dy * dy + dz * dz).sqrt();
                let to_eye = (dx, player.y + bound.height - y, dz);
                let seen = exposure(
                    x,
                    y,
                    z,
                    (player.x - 0.3, player.y, player.z - 0.3),
                    (player.x + 0.3, player.y + bound.height, player.z + 0.3),
                    solid,
                );
                let Some((damage, shove)) = entity_effect(distance, to_eye, power, seen) else {
                    continue;
                };
                motion.insert(player.uuid, shove);

                if player.gamemode == GameMode::Creative {
                    continue;
                }
                let health = (player.health - damage).max(0.0);
                player_registry
                    .update_health(&player.uuid, health, player.food, player.food_saturation)
                    .await;
                channels
                    .dmg_tx
                    .send((player.uuid, health, player.food, player.food_saturation, -1))
                    .ok();
            }

            channels
                .explosion_tx
                .send(ExplosionEvent {
                    x,
                    y,
                    z,
                    power,
                    destroyed: Arc::new(cleared),
                    motion: Arc::new(motion),
                })
                .ok();
        }
    });
}

#[allow(clippy::too_many_arguments)]
async fn spawn_blast_drop(
    (x, y, z): (f64, f64, f64),
    (item_id, count, metadata): (i16, u8, i16),
    entity_tracker: &Arc<RwLock<EntityTracker>>,
    item_spawn_times: &Arc<RwLock<HashMap<i32, Instant>>>,
    item_positions: &Arc<RwLock<DroppedItems>>,
    tracking_range: f64,
    channels: &Channels,
) {
    let entity_id = next_entity_id();
    entity_tracker
        .write()
        .await
        .track(TrackedEntity::item(entity_id, x, y, z, tracking_range));
    item_spawn_times
        .write()
        .await
        .insert(entity_id, Instant::now());
    item_positions
        .write()
        .await
        .insert((entity_id, x, y, z, item_id, count, metadata));

    let mut rng = rand::rng();
    let spread = |rng: &mut rand::rngs::ThreadRng| (rng.random::<f64>() - 0.5) * 0.4;
    channels
        .item_tx
        .send((
            entity_id,
            x,
            y,
            z,
            item_id,
            count,
            metadata,
            spread(&mut rng),
            0.2,
            spread(&mut rng),
        ))
        .ok();
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_falling_task(
    falling: Arc<RwLock<Vec<FallingEntity>>>,
    world: Arc<World>,
    entity_tracker: Arc<RwLock<EntityTracker>>,
    item_spawn_times: Arc<RwLock<HashMap<i32, Instant>>>,
    item_positions: Arc<RwLock<DroppedItems>>,
    tracking_range: f64,
    channels: Channels,
) {
    // Block changes come in on their own loop so a slow tick cannot make the task miss one and leave sand hanging
    {
        let world = world.clone();
        let falling = falling.clone();
        let channels = channels.clone();
        tokio::spawn(async move {
            let mut blocks_rx = channels.block_tx.subscribe();
            while let Ok((x, y, z, _, _)) = blocks_rx.recv().await {
                for check_y in [y, y + 1] {
                    if !(0..=255).contains(&check_y) {
                        continue;
                    }
                    let here = world.get(x, check_y as u8, z).await;
                    let below = world.get(x, (check_y - 1) as u8, z).await;
                    if !should_start_falling(here.id, below.id, check_y) {
                        continue;
                    }
                    world.set(x, check_y as u8, z, Block::air()).await;
                    channels.block_tx.send((x, check_y, z, 0, 0)).ok();

                    let entity = FallingEntity {
                        entity_id: next_entity_id(),
                        kind: coral_server::falling::FallingKind::Block {
                            block_id: here.id,
                            metadata: here.metadata,
                        },
                        x: x as f64 + 0.5,
                        y: check_y as f64,
                        z: z as f64 + 0.5,
                        vx: 0.0,
                        vy: 0.0,
                        vz: 0.0,
                        ticks_alive: 0,
                    };
                    announce_falling(&entity, &channels);
                    falling.write().await.push(entity);
                }
            }
        });
    }

    tokio::spawn(async move {
        let mut lit_rx = channels.tnt_prime_tx.subscribe();
        let mut interval = interval(Duration::from_ticks(1));
        loop {
            tokio::select! {
                Ok((x, y, z, fuse)) = lit_rx.recv() => {
                    let roll = rand::rng().random::<f64>();
                    let (vx, vy, vz) = tnt_launch_velocity(roll);
                    let entity = FallingEntity {
                        entity_id: next_entity_id(),
                        kind: coral_server::falling::FallingKind::PrimedTnt { fuse },
                        x, y, z,
                        vx, vy, vz,
                        ticks_alive: 0
                    };
                    announce_falling(&entity, &channels);
                    falling.write().await.push(entity);
                }
                _ = interval.tick() => {
                    let mut landed = Vec::new();
                    let mut detonated = Vec::new();
                    let mut gone = Vec::new();
                    let mut moves = Vec::new();

                    {
                        let mut list = falling.write().await;
                        for entity in list.iter_mut() {
                            entity.ticks_alive += 1;

                            if let FallingKind::PrimedTnt { fuse } = &mut entity.kind {
                                *fuse = fuse.saturating_sub(1);
                                if *fuse == 0 {
                                    detonated.push((entity.entity_id, entity.x, entity.y, entity.z));
                                    continue;
                                }
                            }
                            let (position, velocity) = falling_step(entity);
                            let below = world.get(position.0.floor() as i32, position.1.floor().clamp(0.0, 255.0) as u8, position.2.floor() as i32).await;

                            if matches!(entity.kind, FallingKind::Block { .. }) && !can_fall_through(below.id) {
                                landed.push(entity.clone());
                                continue;
                            }

                            (entity.x, entity.y, entity.z) = position;
                            (entity.vx, entity.vy, entity.vz) = velocity;

                            if matches!(entity.kind, FallingKind::PrimedTnt { .. }) && !can_fall_through(below.id) {
                                (entity.vx, entity.vy, entity.vz) = falling_bounce(velocity);
                                entity.y = position.1.floor() + 1.0;
                            }

                            if entity.y < 0.0 || entity.ticks_alive > MAX_FALL_TICKS * 6 {
                                gone.push(entity.clone());
                                continue;
                            }

                            moves.push((entity.entity_id, entity.x, entity.y, entity.z));
                        }

                        let finished: Vec<i32> = landed.iter().chain(gone.iter()).map(|e| e.entity_id).chain(detonated.iter().map(|(id, ..)| *id)).collect();
                        list.retain(|e| !finished.contains(&e.entity_id));
                    }

                    for entity in landed {
                        land_falling_block(
                            &entity,
                            &world,
                            &entity_tracker,
                            &item_spawn_times,
                            &item_positions,
                            tracking_range,
                            &channels,
                        ).await;
                    }
                    for (_, x, y, z) in &detonated {
                        channels.explosion_rq_tx.send((*x, *y, *z, TNT_POWER)).ok();
                    }

                    let despawned: Vec<i32> = gone.iter().map(|e| e.entity_id).chain(detonated.iter().map(|(id, ..)| *id)).collect();
                    if !despawned.is_empty() {
                        channels.despawn_tx.send(despawned).ok();
                    }
                    for mv in moves {
                        channels.falling_move_tx.send(mv).ok();
                    }
                }
            }
        }
    });
}

fn announce_falling(entity: &FallingEntity, channels: &Channels) {
    channels
        .falling_spawn_tx
        .send((
            entity.entity_id,
            spawn_object_type(&entity.kind),
            spawn_object_data(&entity.kind),
            entity.x,
            entity.y,
            entity.z,
            entity.vx,
            entity.vy,
            entity.vz,
        ))
        .ok();
}

#[allow(clippy::too_many_arguments)]
async fn land_falling_block(
    entity: &FallingEntity,
    world: &Arc<World>,
    entity_tracker: &Arc<RwLock<EntityTracker>>,
    item_spawn_times: &Arc<RwLock<HashMap<i32, Instant>>>,
    item_positions: &Arc<RwLock<DroppedItems>>,
    tracking_range: f64,
    channels: &Channels,
) {
    let FallingKind::Block { block_id, metadata } = entity.kind else {
        return;
    };
    let (bx, by, bz) = (
        entity.x.floor() as i32,
        entity.y.floor() as i32,
        entity.z.floor() as i32,
    );
    channels.despawn_tx.send(vec![entity.entity_id]).ok();

    let resting = (0..=255).contains(&by);
    let occupied = resting && !can_fall_through(world.get(bx, by as u8, bz).await.id);

    if resting && !occupied {
        world
            .set(bx, by as u8, bz, Block::new(block_id, metadata))
            .await;
        channels
            .block_tx
            .send((bx, by, bz, block_id as i32, metadata))
            .ok();
        return;
    }

    if let Some((item_id, count, item_meta)) = block_drop(block_id, metadata) {
        spawn_blast_drop(
            (entity.x, entity.y, entity.z),
            (item_id, count, item_meta),
            entity_tracker,
            item_spawn_times,
            item_positions,
            tracking_range,
            channels,
        )
        .await;
    }
}
