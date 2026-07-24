use std::{
    collections::HashMap,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use coral_command::{
    CommandContext, CommandDispatcher, CommandResult, list::usage::ResourceMonitor,
};
use coral_protocol::packets::play::{
    chat::builder::ChatBuilder, entity::TileEntity, inventory::ItemStack,
};
use coral_server::{
    experience::XpOrb,
    player::registry::PlayerRegistry,
    projectile::{Projectile, ProjectileKind},
    smelting::{fuel_burn_ticks, smelt_result},
};
use coral_types::{DespawnEntity, ItemInfo, TicksExt, dist_sq3, dist3};
use coral_world::{
    blocks::{Block, WorldBlocks},
    generator::FlatWorldGenerator,
    weather::{Weather, WeatherState},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::{RwLock, broadcast::Sender},
    time::interval,
};
use uuid::Uuid;

use crate::Channels;

pub fn spawn_shutdown_task(
    shutdown_signal: Arc<Sender<()>>,
    player_registry: Arc<PlayerRegistry>,
    world_blocks: Arc<WorldBlocks>,
    world_dir: PathBuf,
    tile_entities: Arc<RwLock<HashMap<(i32, i32, i32), TileEntity>>>,
    generator: Arc<FlatWorldGenerator>,
) {
    tokio::spawn(async move {
        tokio::signal::ctrl_c().await.ok();
        println!("Shutting down, kicking players..");
        shutdown_signal.send(()).ok();

        for _ in 0..50 {
            if player_registry.get_online_count().await == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        println!("Saving world..");
        world_blocks
            .save(&world_dir, &generator, &tile_entities)
            .await;
        println!("World saved. Server closed.");
        std::process::exit(0);
    });
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
    item_positions: Arc<RwLock<HashMap<i32, ItemInfo>>>,
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
            let mut times = item_spawn_times.write().await;
            let mut positions = item_positions.write().await;

            for eid in &expired {
                times.remove(eid);
                positions.remove(eid);
            }
            if !expired.is_empty() {
                despawn_tx.send(expired).ok();
            }
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
                CommandResult::Success(msg) => println!("[CONSOLE] {}", msg),
                CommandResult::Error(msg) => eprintln!("[CONSOLE ERROR] {}", msg),
                CommandResult::Broadcast(msg) => {
                    chat_tx.send(ChatBuilder::plain_json(&msg)).ok();
                }
                CommandResult::None => {}
            }
        }
    });
}
pub fn spawn_world_save_task(
    world_blocks: Arc<WorldBlocks>,
    generator: Arc<FlatWorldGenerator>,
    world_dir: PathBuf,
    tile_entities: Arc<RwLock<HashMap<(i32, i32, i32), TileEntity>>>,
    auto_save_interval: u64,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(auto_save_interval));
        loop {
            interval.tick().await;
            world_blocks
                .save(&world_dir, &generator, &tile_entities)
                .await;
            println!("[World] Auto-Saved.");
        }
    });
}
pub fn spawn_chunk_cache_cleanup_task(world_blocks: Arc<WorldBlocks>) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(60));
        loop {
            interval.tick().await;
            world_blocks
                .evict_stale_chunks(Duration::from_secs(120))
                .await;
        }
    });
}
pub fn spawn_projectile_task(
    projectiles: Arc<RwLock<Vec<Projectile>>>,
    world_blocks: Arc<WorldBlocks>,
    generator: Arc<FlatWorldGenerator>,
    player_registry: Arc<PlayerRegistry>,
    channels: Channels,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_ticks(50));
        loop {
            interval.tick().await;

            let (to_remove, moves, splash_effects) = {
                let mut list = projectiles.write().await;
                let mut to_remove = vec![];
                let mut moves = vec![];
                let mut splash_effects: Vec<(Uuid, u8, u8, i32)> = vec![];

                for proj in list.iter_mut() {
                    // ^^^^^ TODO: future me -> get players inside the chunks where the projectile is
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

                    let block = world_blocks
                        .get(bx, by.clamp(0, 255) as u8, bz, &generator)
                        .await;
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
                                let players = player_registry.get_all().await;
                                for target in &players {
                                    let dist =
                                        dist3(target.x, target.y, target.z, proj.x, proj.y, proj.z);

                                    if dist <= 4.0 {
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
                        let players = player_registry.get_all().await;
                        for target in &players {
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
                                let speed =
                                    (proj.vx.powi(2) + proj.vy.powi(2) + proj.vz.powi(2)).sqrt();
                                let damage = (speed * 3.0).clamp(2.0, 10.0) as f32;
                                let new_health = (target.health - damage).max(0.0);

                                player_registry
                                    .update_health(
                                        &target.uuid,
                                        target.health,
                                        target.food,
                                        target.food_saturation,
                                    )
                                    .await;
                                channels
                                    .dmg_tx
                                    .send((
                                        target.uuid,
                                        new_health,
                                        target.food,
                                        target.food_saturation,
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

                    moves.push((proj.entity_id, proj.x, proj.y, proj.z));
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
    world_blocks: Arc<WorldBlocks>,
    generator: Arc<FlatWorldGenerator>,
    player_registry: Arc<PlayerRegistry>,
    channels: Channels,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_ticks(50));
        loop {
            interval.tick().await;

            let (moves, picked_up_per_player) = {
                let mut orbs = xp_orbs.write().await;
                let mut moves = vec![];
                let mut to_remove = vec![];
                let mut pickups: Vec<(Uuid, i32)> = vec![];

                let players = player_registry.get_all().await;
                for orb in orbs.iter_mut() {
                    orb.ticks_alive += 1;
                    orb.vy -= 0.01;
                    orb.vy = orb.vy.clamp(-0.3, 0.3);
                    orb.y += orb.vy;

                    let bx = orb.x.floor() as i32;
                    let by = (orb.y.floor() as i32).clamp(0, 255) as u8;
                    let bz = orb.z.floor() as i32;
                    let below = world_blocks
                        .get(bx, by.saturating_sub(1), bz, &generator)
                        .await;
                    if !below.is_air() && orb.vy < 0.0 {
                        orb.vy = 0.0;
                    }

                    let mut picked = false;
                    for p in &players {
                        if p.is_dead {
                            continue;
                        }
                        if dist3(p.x, p.y, p.z, orb.x, orb.y, orb.z) < 1.0 {
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
                channels.despawn_tx.send(picked_up_per_player.0).ok();
            }
            for (uuid, amount) in picked_up_per_player.1 {
                channels.xp_pickup_tx.send((uuid, amount)).ok();
                channels
                    .sound_tx
                    .send(("random.orb".to_string(), 0.0, 0.0, 0.0, 0.5, 63))
                    .ok();
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
    tile_entities: Arc<RwLock<HashMap<(i32, i32, i32), TileEntity>>>,
    world_blocks: Arc<WorldBlocks>,
    generator: Arc<FlatWorldGenerator>,
    channels: Channels,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_ticks(1));
        loop {
            interval.tick().await;

            let mut storage = tile_entities.write().await;
            for (pos, tile) in storage.iter_mut() {
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
                    .and_then(|i| smelt_result(i.item_id))
                    .map(|(rid, rmeta)| match output {
                        None => true,
                        Some(o) => o.item_id == rid && o.metadata == rmeta && o.count < 64,
                    })
                    .unwrap_or(false);
                if *burn_ticks == 0 && can_smelt {
                    if let Some(f) = fuel
                        && let Some(burn) = fuel_burn_ticks(f.item_id)
                    {
                        *burn_ticks = burn;
                        *burn_ticks_total = burn;
                        f.count -= 1;
                        if f.count == 0 {
                            *fuel = None;
                        }
                    }
                }

                let mut changed = false;

                if *burn_ticks > 0 {
                    *burn_ticks -= 1;
                    if can_smelt {
                        *cook_ticks += 1;
                        if *cook_ticks >= 200 {
                            *cook_ticks = 0;
                            let (rid, rmeta) =
                                smelt_result(input.as_ref().unwrap().item_id).unwrap();
                            match output {
                                Some(o) => o.count += 1,
                                None => {
                                    *output = Some(ItemStack {
                                        item_id: rid,
                                        count: 1,
                                        metadata: rmeta,
                                        durability: 0,
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
                    let current = world_blocks
                        .get(pos.0, pos.1 as u8, pos.2, &generator)
                        .await;
                    let new_id = if now_burning { 62 } else { 61 };
                    world_blocks
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
