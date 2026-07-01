use std::{
    collections::HashMap,
    io::ErrorKind,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use coral_protocol::packets::{PacketRegistry, play::chat::builder::ChatBuilder};
use coral_types::GamemodeUpdate;
use rsa::RsaPrivateKey;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpListener,
    sync::{
        RwLock,
        broadcast::{Sender, channel},
    },
    time::interval,
};
use uuid::Uuid;

use coral_command::{
    CommandContext, CommandDispatcher, CommandResult, gamemode_command, list_command,
    version_command,
};
use coral_config::Config;
use coral_protocol::encryption::generate_rsa_key;
use coral_server::{
    banlist::BanList,
    entity_tracker::EntityTracker,
    experience::XpOrb,
    items::ItemRegistry,
    ops::OpsFile,
    player::Player,
    projectile::{Projectile, ProjectileKind},
    registry::PlayerRegistry,
    whitelist::WhitelistFile,
};
use coral_world::{
    blocks::{WorldBlocks, registry::BlockRegistry},
    generator::FlatWorldGenerator,
    level::{read_spawn_point, write_level_dat},
    weather::{Weather, WeatherState},
};

mod codec;

#[derive(Clone)]
pub struct ServerContext {
    packet_registry: Arc<PacketRegistry>,
    player_registry: Arc<PlayerRegistry>,
    item_registry: Arc<ItemRegistry>,
    block_registry: Arc<BlockRegistry>,
    server_icon: Arc<Option<String>>,
    config: Arc<Config>,
    dispatcher: Arc<CommandDispatcher>,
    entity_tracker: Arc<RwLock<EntityTracker>>,
    item_spawn_times: Arc<RwLock<HashMap<i32, Instant>>>,
    item_positions: Arc<RwLock<HashMap<i32, ItemInfo>>>,
    projectiles: Arc<RwLock<Vec<Projectile>>>,
    channels: Channels,
    world_blocks: Arc<WorldBlocks>,
    generator: Arc<FlatWorldGenerator>,
    private_key: Arc<RsaPrivateKey>,
    public_key_der: Arc<Vec<u8>>,
    ops: Arc<RwLock<OpsFile>>,
    whitelist: Arc<RwLock<WhitelistFile>>,
    banlist: Arc<RwLock<BanList>>,
    spawn_point: Arc<RwLock<(f64, f64, f64)>>,
    world_dir: Arc<PathBuf>,
    xp_orbs: Arc<RwLock<Vec<XpOrb>>>,
}

type PositionUpdate = (Uuid, i32, f64, f64, f64, f32, f32, bool);
type JoinLeave = (Player, bool);
type PingUpdate = (Uuid, i32);
type BlockUpdate = (i32, i32, i32, i32, u8);
type BreakAnimation = (i32, i32, i32, i32, u8);
type AnimationUpdate = (i32, u8);
type MetadataUpdate = (i32, u8, u8);
type DamageEvent = (Uuid, f32, i32, f32, i32);
type ItemDrop = (i32, f64, f64, f64, i16, u8, i16);
type DespawnEntity = i32;
type ItemInfo = (i32, f64, f64, f64, i16, u8, i16);
type ItemPickup = (i32, Uuid, i32);
type TimeUpdate = (i64, i64);
type WeatherUpdate = WeatherState;
type EntityStatusUpdate = (i32, u8);
type EquipmentUpdate = (i32, i16, i16, u8, i16);
type SoundEffect = (String, f64, f64, f64, f32, u8);
type ParticleEffect = (i32, i32, f32, f32, f32, f32, f32, f32, f32, i32);
type ProjectileSpawn = (i32, i32, ProjectileKind, f64, f64, f64, f64, f64, f64);
type ProjectileMove = (i32, f64, f64, f64);
type SplashEffect = (Uuid, u8, u8, i32);
type XpOrbSpawn = (i32, f64, f64, f64, i32);
type XpOrbMove = (i32, f64, f64, f64);
type XpPickup = (Uuid, i32);

#[derive(Clone)]
pub struct Channels {
    chat_tx: Arc<Sender<String>>,
    join_tx: Arc<Sender<JoinLeave>>,
    pos_tx: Arc<Sender<PositionUpdate>>,
    gm_tx: Arc<Sender<GamemodeUpdate>>,
    ping_tx: Arc<Sender<PingUpdate>>,
    block_tx: Arc<Sender<BlockUpdate>>,
    break_tx: Arc<Sender<BreakAnimation>>,
    anim_tx: Arc<Sender<AnimationUpdate>>,
    meta_tx: Arc<Sender<MetadataUpdate>>,
    dmg_tx: Arc<Sender<DamageEvent>>,
    item_tx: Arc<Sender<ItemDrop>>,
    despawn_tx: Arc<Sender<DespawnEntity>>,
    pickup_tx: Arc<Sender<ItemPickup>>,
    time_tx: Arc<Sender<TimeUpdate>>,
    weather_tx: Arc<Sender<WeatherUpdate>>,
    tick_tx: Arc<Sender<()>>,
    status_tx: Arc<Sender<EntityStatusUpdate>>,
    equip_tx: Arc<Sender<EquipmentUpdate>>,
    sound_tx: Arc<Sender<SoundEffect>>,
    shutdown_tx: Arc<Sender<()>>,
    particle_tx: Arc<Sender<ParticleEffect>>,
    projectile_spawn_tx: Arc<Sender<ProjectileSpawn>>,
    projectile_move_tx: Arc<Sender<ProjectileMove>>,
    splash_effect_tx: Arc<Sender<SplashEffect>>,
    xp_orb_spawn_tx: Arc<Sender<XpOrbSpawn>>,
    xp_orb_move_tx: Arc<Sender<XpOrbMove>>,
    xp_pickup_tx: Arc<Sender<XpPickup>>,
}
impl Channels {
    pub fn new() -> Self {
        Self {
            chat_tx: Arc::new(channel::<String>(50).0),
            join_tx: Arc::new(channel::<JoinLeave>(16).0),
            pos_tx: Arc::new(channel::<PositionUpdate>(100).0),
            gm_tx: Arc::new(channel::<GamemodeUpdate>(16).0),
            ping_tx: Arc::new(channel::<PingUpdate>(16).0),
            block_tx: Arc::new(channel::<BlockUpdate>(100).0),
            break_tx: Arc::new(channel::<BreakAnimation>(100).0),
            anim_tx: Arc::new(channel::<AnimationUpdate>(100).0),
            meta_tx: Arc::new(channel::<MetadataUpdate>(100).0),
            dmg_tx: Arc::new(channel::<DamageEvent>(100).0),
            item_tx: Arc::new(channel::<ItemDrop>(1000).0),
            despawn_tx: Arc::new(channel::<DespawnEntity>(100).0),
            pickup_tx: Arc::new(channel::<ItemPickup>(100).0),
            time_tx: Arc::new(channel::<TimeUpdate>(1).0),
            weather_tx: Arc::new(channel::<WeatherUpdate>(1).0),
            tick_tx: Arc::new(channel(4).0),
            status_tx: Arc::new(channel::<EntityStatusUpdate>(100).0),
            equip_tx: Arc::new(channel::<EquipmentUpdate>(100).0),
            sound_tx: Arc::new(channel::<SoundEffect>(100).0),
            shutdown_tx: Arc::new(channel::<()>(1).0),
            particle_tx: Arc::new(channel::<ParticleEffect>(100).0),
            projectile_spawn_tx: Arc::new(channel::<ProjectileSpawn>(100).0),
            projectile_move_tx: Arc::new(channel::<ProjectileMove>(200).0),
            splash_effect_tx: Arc::new(channel::<SplashEffect>(100).0),
            xp_orb_spawn_tx: Arc::new(channel::<XpOrbSpawn>(100).0),
            xp_orb_move_tx: Arc::new(channel::<XpOrbMove>(200).0),
            xp_pickup_tx: Arc::new(channel::<XpPickup>(100).0),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(coral_config::Config::load());
    let addr = format!("0.0.0.0:{}", config.server.port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => {
            println!("Minecraft Server 1.8.x started at {}", addr);
            l
        }
        Err(e) => {
            if e.kind() == ErrorKind::AddrInUse {
                eprintln!("Port {} is already in use!", config.server.port);
            } else {
                eprintln!("Failed to bind a port to {}: {}", addr, e);
            }
            std::process::exit(1);
        }
    };

    let server_icon = load_server_icon_file()
        .inspect(|_| println!("Server icon loaded successfully"))
        .or_else(|| {
            println!("No server icon found or invalid size");
            None
        });

    let player_registry = Arc::new(PlayerRegistry::new());
    let channels = Channels::new();

    let dispatcher = Arc::new(CommandDispatcher::new());
    dispatcher.register(version_command()).await;
    dispatcher
        .register(list_command(player_registry.clone()))
        .await;
    dispatcher
        .register(gamemode_command(
            player_registry.clone(),
            channels.gm_tx.clone(),
        ))
        .await;

    let (private_key, public_key_der) = generate_rsa_key();

    let world_dir = std::path::Path::new(&config.world.world_name);
    let spawn_point = read_spawn_point(world_dir).await.unwrap_or((0.5, 5.0, 0.5));

    let ctx = ServerContext {
        packet_registry: Arc::new(PacketRegistry::new()),
        server_icon: Arc::new(server_icon),
        item_registry: Arc::new(ItemRegistry::new()),
        block_registry: Arc::new(BlockRegistry::new()),
        config: config.clone(),
        dispatcher,
        entity_tracker: Arc::new(RwLock::new(EntityTracker::new())),
        item_spawn_times: Arc::new(RwLock::new(HashMap::new())),
        item_positions: Arc::new(RwLock::new(HashMap::new())),
        projectiles: Arc::new(RwLock::new(Vec::new())),
        channels,
        world_blocks: Arc::new(WorldBlocks::new()),
        generator: Arc::new(FlatWorldGenerator::new()),
        player_registry,
        private_key: Arc::new(private_key),
        public_key_der: Arc::new(public_key_der),
        ops: Arc::new(RwLock::new(OpsFile::load())),
        whitelist: Arc::new(RwLock::new(WhitelistFile::load())),
        banlist: Arc::new(RwLock::new(BanList::load())),
        spawn_point: Arc::new(RwLock::new(spawn_point)),
        world_dir: Arc::new(world_dir.to_path_buf()),
        xp_orbs: Arc::new(RwLock::new(Vec::new())),
    };

    ctx.world_blocks.load(world_dir, &ctx.generator).await;

    if !world_dir.join("level.dat").exists() {
        write_level_dat(world_dir, "world");
    }

    if config.world.enable_auto_save {
        spawn_world_save_task(
            ctx.world_blocks.clone(),
            ctx.generator.clone(),
            world_dir.to_path_buf(),
            config.world.auto_save_interval,
        );
    }

    spawn_console_task(ctx.dispatcher.clone(), ctx.channels.chat_tx.clone());
    spawn_shutdown_task(
        ctx.channels.shutdown_tx.clone(),
        ctx.player_registry.clone(),
        ctx.world_blocks.clone(),
        world_dir.to_path_buf(),
        ctx.generator.clone(),
    );
    spawn_tick_task(ctx.channels.tick_tx.clone(), ctx.player_registry.clone());
    spawn_world_time_task(ctx.channels.time_tx.clone());

    if !config.world.disable_weather {
        spawn_weather_task(ctx.channels.weather_tx.clone());
    }

    spawn_item_despawn_task(
        ctx.channels.despawn_tx.clone(),
        config.world.item_despawn_seconds,
        ctx.item_spawn_times.clone(),
        ctx.item_positions.clone(),
    );

    spawn_projectile_task(
        ctx.projectiles.clone(),
        ctx.world_blocks.clone(),
        ctx.generator.clone(),
        ctx.player_registry.clone(),
        ctx.channels.clone(),
    );

    spawn_chunk_cache_cleanup_task(ctx.world_blocks.clone());

    spawn_xp_orb_task(
        ctx.xp_orbs.clone(),
        ctx.world_blocks.clone(),
        ctx.generator.clone(),
        ctx.player_registry.clone(),
        ctx.channels.clone(),
    );

    loop {
        let (socket, _) = listener.accept().await?;
        let ctx = ctx.clone();

        tokio::spawn(async move {
            codec::process(socket, ctx).await;
        });
    }
}

fn spawn_shutdown_task(
    shutdown_signal: Arc<Sender<()>>,
    player_registry: Arc<PlayerRegistry>,
    world_blocks: Arc<WorldBlocks>,
    world_dir: PathBuf,
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
        world_blocks.save(&world_dir, &generator).await;
        println!("World saved. Server closed.");
        std::process::exit(0);
    });
}
fn spawn_tick_task(tick_tx: Arc<Sender<()>>, player_registry: Arc<PlayerRegistry>) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_millis(50));
        loop {
            interval.tick().await;
            player_registry.tick().await;
            tick_tx.send(()).ok();
        }
    });
}
fn spawn_world_time_task(time_tx: Arc<Sender<(i64, i64)>>) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_millis(50)); // 1 tick
        let mut world_age: i64 = 0;
        let mut time_of_day: i64 = 0;

        loop {
            interval.tick().await;
            world_age += 1;
            time_of_day = (time_of_day + 1) % 24000;

            if world_age % 20 == 0 {
                time_tx.send((world_age, time_of_day)).ok();
            }
        }
    });
}
fn spawn_weather_task(weather_tx: Arc<Sender<WeatherState>>) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_millis(50));
        let mut weather = Weather::new();

        loop {
            interval.tick().await;
            if let Some(new_state) = weather.tick() {
                weather_tx.send(new_state).ok();
            }
        }
    });
}
fn spawn_item_despawn_task(
    despawn_tx: Arc<Sender<i32>>,
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

            for eid in expired {
                times.remove(&eid);
                positions.remove(&eid);
                despawn_tx.send(eid).ok();
            }
        }
    });
}
fn spawn_console_task(dispatcher: Arc<CommandDispatcher>, chat_tx: Arc<Sender<String>>) {
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
    auto_save_interval: u64,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(auto_save_interval));
        loop {
            interval.tick().await;
            world_blocks.save(&world_dir, &generator).await;
            println!("[World] Auto-Saved.");
        }
    });
}
fn spawn_chunk_cache_cleanup_task(world_blocks: Arc<WorldBlocks>) {
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
#[allow(clippy::too_many_arguments)]
fn spawn_projectile_task(
    projectiles: Arc<RwLock<Vec<Projectile>>>,
    world_blocks: Arc<WorldBlocks>,
    generator: Arc<FlatWorldGenerator>,
    player_registry: Arc<PlayerRegistry>,
    channels: Channels,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_millis(50));
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
                                    let dx = target.x - proj.x;
                                    let dy = target.y - proj.y;
                                    let dz = target.z - proj.z;
                                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();

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
                            let dx = target.x - proj.x;
                            let dy = target.y - proj.y;
                            let dz = target.z - proj.z;
                            let dist_sq = dx * dx + dy * dy + dz * dz;

                            if dist_sq < 0.64 {
                                let speed =
                                    (proj.vx.powi(2) + proj.vy.powi(2) + proj.vz.powi(2)).sqrt();
                                let damage = (speed * 3.0).clamp(2.0, 10.0) as f32;
                                let new_health = (target.health - damage).max(0.0);

                                player_registry
                                    .update_health(
                                        target.uuid,
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

                    if proj.ticks_alive > 600 {
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
            for eid in to_remove {
                channels.despawn_tx.send(eid).ok();
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
fn spawn_xp_orb_task(
    xp_orbs: Arc<RwLock<Vec<XpOrb>>>,
    world_blocks: Arc<WorldBlocks>,
    generator: Arc<FlatWorldGenerator>,
    player_registry: Arc<PlayerRegistry>,
    channels: Channels,
) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_millis(50));
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
                        let dx = p.x - orb.x;
                        let dy = p.y - orb.y;
                        let dz = p.z - orb.z;
                        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                        if dist < 1.0 {
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
            for eid in picked_up_per_player.0 {
                channels.despawn_tx.send(eid).ok();
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

// TODO: map error
fn load_server_icon_file() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    let icon_path = cwd.join("server-icon.png");
    let bytes = std::fs::read(&icon_path).ok()?;

    if bytes.len() > 24 {
        let width = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
        let height = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
        if width != 64 || height != 64 {
            eprintln!("server-icon.png must be 64x64, got {}x{}", width, height);
            return None;
        }
    }

    Some(format!("data:image/png;base64,{}", STANDARD.encode(&bytes)))
}
