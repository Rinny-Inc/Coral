use std::{
    collections::HashMap,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use coral_protocol::packets::{PacketRegistry, play::chat::builder::ChatBuilder};
use rsa::RsaPrivateKey;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::TcpListener,
    sync::{RwLock, broadcast},
    time::interval,
};
use uuid::Uuid;

use coral_command::{CommandContext, CommandDispatcher, CommandResult, version_command};
use coral_config::Config;
use coral_protocol::encryption::generate_rsa_key;
use coral_server::{
    banlist::BanList, entity_tracker::EntityTracker, items::ItemRegistry, ops::OpsFile,
    player::Player, registry::PlayerRegistry, whitelist::WhitelistFile,
};
use coral_world::{
    blocks::{WorldBlocks, registry::BlockRegistry},
    generator::FlatWorldGenerator,
    level::{read_spawn_point, write_level_dat},
    weather::{Weather, WeatherState},
};

mod codec;

type PositionUpdate = (Uuid, i32, f64, f64, f64, f32, f32, bool);
type JoinLeave = (Player, bool);
type GamemodeUpdate = (Uuid, u8);
type PingUpdate = (Uuid, u32);
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
    chat_tx: Arc<broadcast::Sender<String>>,
    join_tx: Arc<broadcast::Sender<JoinLeave>>,
    pos_tx: Arc<broadcast::Sender<PositionUpdate>>,
    gm_tx: Arc<broadcast::Sender<GamemodeUpdate>>,
    ping_tx: Arc<broadcast::Sender<PingUpdate>>,
    block_tx: Arc<broadcast::Sender<BlockUpdate>>,
    break_tx: Arc<broadcast::Sender<BreakAnimation>>,
    anim_tx: Arc<broadcast::Sender<AnimationUpdate>>,
    meta_tx: Arc<broadcast::Sender<MetadataUpdate>>,
    dmg_tx: Arc<broadcast::Sender<DamageEvent>>,
    item_tx: Arc<broadcast::Sender<ItemDrop>>,
    despawn_tx: Arc<broadcast::Sender<DespawnEntity>>,
    pickup_tx: Arc<broadcast::Sender<ItemPickup>>,
    time_tx: Arc<broadcast::Sender<TimeUpdate>>,
    weather_tx: Arc<broadcast::Sender<WeatherUpdate>>,
    tick_tx: Arc<broadcast::Sender<()>>,
    status_tx: Arc<broadcast::Sender<EntityStatusUpdate>>,
    equip_tx: Arc<broadcast::Sender<EquipmentUpdate>>,
    sound_tx: Arc<broadcast::Sender<SoundEffect>>,
    shutdown_tx: Arc<broadcast::Sender<()>>,
    particle_tx: Arc<broadcast::Sender<ParticleEffect>>,
    world_blocks: Arc<WorldBlocks>,
    generator: Arc<FlatWorldGenerator>,
    private_key: Arc<RsaPrivateKey>,
    public_key_der: Arc<Vec<u8>>,
    ops: Arc<RwLock<OpsFile>>,
    whitelist: Arc<RwLock<WhitelistFile>>,
    banlist: Arc<RwLock<BanList>>,
    spawn_point: Arc<RwLock<(f64, f64, f64)>>,
    world_dir: Arc<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(coral_config::Config::load());
    let addr = format!("0.0.0.0:{}", config.server.port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => {
            println!("Minecraft Server 1.8.9 started at {}", addr);
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

    let server_icon = load_server_icon_file();
    match &server_icon {
        Some(_) => println!("Server icon loaded successfully"),
        None => println!("No server icon found or invalid size"),
    }

    let dispatcher = Arc::new(CommandDispatcher::new());
    dispatcher.register(version_command()).await;

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
        chat_tx: Arc::new(broadcast::channel::<String>(50).0),
        join_tx: Arc::new(broadcast::channel::<JoinLeave>(16).0),
        pos_tx: Arc::new(broadcast::channel::<PositionUpdate>(100).0),
        gm_tx: Arc::new(broadcast::channel::<GamemodeUpdate>(16).0),
        ping_tx: Arc::new(broadcast::channel::<PingUpdate>(16).0),
        block_tx: Arc::new(broadcast::channel::<BlockUpdate>(100).0),
        break_tx: Arc::new(broadcast::channel::<BreakAnimation>(100).0),
        anim_tx: Arc::new(broadcast::channel::<AnimationUpdate>(100).0),
        meta_tx: Arc::new(broadcast::channel::<MetadataUpdate>(100).0),
        dmg_tx: Arc::new(broadcast::channel::<DamageEvent>(100).0),
        item_tx: Arc::new(broadcast::channel::<ItemDrop>(1000).0),
        despawn_tx: Arc::new(broadcast::channel::<DespawnEntity>(100).0),
        pickup_tx: Arc::new(broadcast::channel::<ItemPickup>(100).0),
        time_tx: Arc::new(broadcast::channel::<TimeUpdate>(1).0),
        weather_tx: Arc::new(broadcast::channel::<WeatherUpdate>(1).0),
        tick_tx: Arc::new(broadcast::channel(4).0),
        status_tx: Arc::new(broadcast::channel::<EntityStatusUpdate>(100).0),
        equip_tx: Arc::new(broadcast::channel::<EquipmentUpdate>(100).0),
        sound_tx: Arc::new(broadcast::channel::<SoundEffect>(100).0),
        shutdown_tx: Arc::new(broadcast::channel::<()>(1).0),
        particle_tx: Arc::new(broadcast::channel::<ParticleEffect>(100).0),
        world_blocks: Arc::new(WorldBlocks::new()),
        generator: Arc::new(FlatWorldGenerator::new()),
        player_registry: Arc::new(PlayerRegistry::new()),
        private_key: Arc::new(private_key),
        public_key_der: Arc::new(public_key_der),
        ops: Arc::new(RwLock::new(OpsFile::load())),
        whitelist: Arc::new(RwLock::new(WhitelistFile::load())),
        banlist: Arc::new(RwLock::new(BanList::load())),
        spawn_point: Arc::new(RwLock::new(spawn_point)),
        world_dir: Arc::new(world_dir.to_path_buf()),
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

    spawn_console_task(ctx.dispatcher.clone(), ctx.chat_tx.clone());
    spawn_shutdown_task(
        ctx.shutdown_tx.clone(),
        ctx.player_registry.clone(),
        ctx.world_blocks.clone(),
        world_dir.to_path_buf(),
        ctx.generator.clone(),
    );
    spawn_tick_task(ctx.tick_tx.clone(), ctx.player_registry.clone());
    spawn_world_time_task(ctx.time_tx.clone());

    if !config.world.disable_weather {
        spawn_weather_task(ctx.weather_tx.clone());
    }

    spawn_item_despawn_task(
        ctx.despawn_tx.clone(),
        config.world.item_despawn_seconds,
        ctx.item_spawn_times.clone(),
        ctx.item_positions.clone(),
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
    shutdown_signal: Arc<broadcast::Sender<()>>,
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
fn spawn_tick_task(tick_tx: Arc<broadcast::Sender<()>>, player_registry: Arc<PlayerRegistry>) {
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_millis(50));
        loop {
            interval.tick().await;
            player_registry.tick().await;
            tick_tx.send(()).ok();
        }
    });
}
fn spawn_world_time_task(time_tx: Arc<broadcast::Sender<(i64, i64)>>) {
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
fn spawn_weather_task(weather_tx: Arc<broadcast::Sender<WeatherState>>) {
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
    despawn_tx: Arc<broadcast::Sender<i32>>,
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
fn spawn_console_task(dispatcher: Arc<CommandDispatcher>, chat_tx: Arc<broadcast::Sender<String>>) {
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
