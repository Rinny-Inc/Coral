use std::{
    collections::{HashMap, HashSet},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use coral::{Channels, register_commands, world_path};
use coral_protocol::packets::{PacketRegistry, play::entity::TileEntity};
use coral_types::{BreakAnimation, EquipmentUpdate, ItemInfo, ItemPickup, SoundEffect};
use rsa::RsaPrivateKey;
use tokio::{
    net::TcpListener,
    sync::{RwLock, broadcast::channel},
};

use coral_command::{CommandDispatcher, list::usage::ResourceMonitor};
use coral_config::Config;
use coral_protocol::encryption::generate_rsa_key;
use coral_server::{
    banlist::BanList,
    entity_tracker::EntityTracker,
    experience::XpOrb,
    items::ItemRegistry,
    ops::OpsFile,
    player::registry::PlayerRegistry,
    projectile::Projectile,
    scoreboard::{ScoreboardManager, team::TeamManager},
    statistics::StatTracker,
    whitelist::WhitelistFile,
};
use coral_world::{
    blocks::registry::BlockRegistry,
    level::{read_spawn_point, write_level_dat},
    world::World,
};

use crate::fluid_sim::FluidQueue;

mod codec;
mod fluid_sim;
mod tasks;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(coral_config::Config::load());

    let addr = format!("0.0.0.0:{}", config.server.port);
    let listener = match bind_with_retry(&addr).await {
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

    let restart_tx = Arc::new(channel::<PathBuf>(1).0);
    tokio::spawn(coral::updater::run(
        config.updater.clone(),
        restart_tx.clone(),
    ));

    let cwd = std::env::current_dir()?;

    let server_icon = load_server_icon_file(&cwd)
        .inspect(|_| println!("Server icon loaded successfully"))
        .or_else(|| {
            println!("No server icon found or invalid size");
            None
        });

    let resource_monitor = Arc::new(ResourceMonitor::new());

    let world_path = world_path(&config.world.world_name)?;
    let world_dir = cwd.join(&config.world.world_name);

    let generator = coral_world::generator::from_name(&config.world.generator);
    println!("[World] generator: {}", generator.name());
    if generator.is_empty() {
        println!(
            "[World] this world generates nothing! players will fall unless there is \
            something saved under the spawn point"
        );
    }

    if !world_dir.join("level.dat").exists() {
        write_level_dat(
            world_path,
            &config.world.world_name,
            generator.spawn_y() as i32,
        );
    }

    let spawn_point =
        read_spawn_point(world_path)
            .await
            .unwrap_or((0.5, generator.spawn_y(), 0.5, 0.0, 0.0));

    let world = Arc::new(World::new(generator));
    world.load(world_path).await;

    let (private_key, public_key_der) = generate_rsa_key();

    let ctx = ServerContext::new(
        server_icon,
        config.clone(),
        world,
        private_key,
        public_key_der,
        spawn_point,
        world_dir,
    );

    register_commands(
        &ctx.dispatcher,
        &ctx.channels,
        &ctx.player_registry,
        &ctx.ops,
        &ctx.whitelist,
        &resource_monitor,
        &ctx.spawn_point,
        &ctx.world_dir,
        &ctx.world.time,
    )
    .await;

    tasks::spawn_furnace_task(
        ctx.tile_entities.clone(),
        ctx.world.clone(),
        ctx.channels.clone(),
    );

    if config.world.enable_auto_save {
        tasks::spawn_world_save_task(
            ctx.world.clone(),
            ctx.world_dir.to_path_buf(),
            ctx.tile_entities.clone(),
            config.world.auto_save_interval,
        );
    }

    tasks::spawn_console_task(ctx.dispatcher.clone(), ctx.channels.chat_tx.clone());
    tasks::spawn_shutdown_task(
        ctx.channels.shutdown_tx.clone(),
        restart_tx,
        ctx.channels.chat_tx.clone(),
        Duration::from_secs(config.updater.restart_grace_seconds),
        coral::updater::plan_restart(&config.updater.restart_command),
        ctx.player_registry.clone(),
        ctx.world.clone(),
        ctx.world_dir.to_path_buf(),
        ctx.tile_entities.clone(),
    );
    tasks::spawn_tick_task(ctx.channels.tick_tx.clone(), ctx.player_registry.clone());
    tasks::spawn_world_time_task(
        ctx.channels.time_tx.clone(),
        ctx.player_registry.clone(),
        ctx.channels.wake_tx.clone(),
    );

    if !config.world.disable_weather {
        tasks::spawn_weather_task(ctx.channels.weather_tx.clone());
    }

    tasks::spawn_item_despawn_task(
        ctx.channels.despawn_tx.clone(),
        config.world.item_despawn_seconds,
        ctx.item_spawn_times.clone(),
        ctx.item_positions.clone(),
    );

    tasks::spawn_projectile_task(
        ctx.projectiles.clone(),
        ctx.world.clone(),
        ctx.player_registry.clone(),
        ctx.channels.clone(),
    );

    tasks::spawn_chunk_cache_cleanup_task(ctx.world.clone());

    tasks::spawn_xp_orb_task(
        ctx.xp_orbs.clone(),
        ctx.world.clone(),
        ctx.player_registry.clone(),
        ctx.channels.clone(),
    );

    fluid_sim::spawn_fluid_task(
        ctx.fluid_queue.clone(),
        ctx.world.clone(),
        ctx.channels.clone(),
    );

    tasks::spawn_resource_monitor_task(resource_monitor.clone());

    loop {
        let (socket, _) = listener.accept().await?;
        let ctx = ctx.clone();

        tokio::spawn(async move {
            codec::process(socket, ctx).await;
        });
    }
}

#[derive(Clone)]
pub struct ServerContext {
    packet_registry: Arc<PacketRegistry>,
    player_registry: Arc<PlayerRegistry>,
    item_registry: Arc<ItemRegistry>,
    block_registry: Arc<BlockRegistry>,
    // TODO: mob_registry: Arc<MobRegistry>,
    server_icon: Arc<Option<String>>,
    config: Arc<Config>,
    dispatcher: Arc<CommandDispatcher>,
    entity_tracker: Arc<RwLock<EntityTracker>>,
    item_spawn_times: Arc<RwLock<HashMap<i32, Instant>>>,
    item_positions: Arc<RwLock<HashMap<i32, ItemInfo>>>,
    projectiles: Arc<RwLock<Vec<Projectile>>>,
    channels: Channels,
    world: Arc<World>,
    private_key: Arc<RsaPrivateKey>,
    public_key_der: Arc<Vec<u8>>,
    ops: Arc<RwLock<OpsFile>>,
    whitelist: Arc<RwLock<WhitelistFile>>,
    banlist: Arc<RwLock<BanList>>,
    spawn_point: Arc<RwLock<(f64, f64, f64, f32, f32)>>,
    world_dir: Arc<PathBuf>,
    xp_orbs: Arc<RwLock<Vec<XpOrb>>>,
    fluid_queue: Arc<RwLock<FluidQueue>>,
    tile_entities: Arc<RwLock<HashMap<(i32, i32, i32), TileEntity>>>,
    server_loaded_chunks: Arc<RwLock<HashSet<(i32, i32)>>>,
    scoreboard: Arc<ScoreboardManager>,
    teams: Arc<TeamManager>,
    stats: Arc<StatTracker>,
}
impl ServerContext {
    fn new(
        server_icon: Option<String>,
        config: Arc<Config>,
        world: Arc<World>,
        private_key: RsaPrivateKey,
        public_key_der: Vec<u8>,
        spawn_point: (f64, f64, f64, f32, f32),
        world_dir: PathBuf,
    ) -> Self {
        Self {
            packet_registry: Arc::new(PacketRegistry::new()),
            server_icon: Arc::new(server_icon),
            item_registry: Arc::new(ItemRegistry::new()),
            block_registry: Arc::new(BlockRegistry::new()),
            config,
            dispatcher: Arc::new(CommandDispatcher::new()),
            entity_tracker: Arc::new(RwLock::new(EntityTracker::new())),
            item_spawn_times: Arc::new(RwLock::new(HashMap::new())),
            item_positions: Arc::new(RwLock::new(HashMap::new())),
            projectiles: Arc::new(RwLock::new(Vec::new())),
            channels: Channels::new(),
            world,
            player_registry: Arc::new(PlayerRegistry::new()),
            private_key: Arc::new(private_key),
            public_key_der: Arc::new(public_key_der),
            ops: Arc::new(RwLock::new(OpsFile::load())),
            whitelist: Arc::new(RwLock::new(WhitelistFile::load())),
            banlist: Arc::new(RwLock::new(BanList::load())),
            spawn_point: Arc::new(RwLock::new(spawn_point)),
            world_dir: Arc::new(world_dir),
            xp_orbs: Arc::new(RwLock::new(Vec::new())),
            fluid_queue: Arc::new(RwLock::new(FluidQueue::new())),
            tile_entities: Arc::new(RwLock::new(HashMap::new())),
            server_loaded_chunks: Arc::new(RwLock::new(HashSet::new())),
            scoreboard: Arc::new(ScoreboardManager::new()),
            teams: Arc::new(TeamManager::new()),
            stats: Arc::new(StatTracker::new()),
        }
    }
}

fn load_server_icon_file(cwd: &Path) -> Option<String> {
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

async fn bind_with_retry(addr: &str) -> std::io::Result<TcpListener> {
    const ATTEMPTS: u32 = 12;
    for attempt in 1..=ATTEMPTS {
        match TcpListener::bind(addr).await {
            Ok(l) => return Ok(l),
            Err(e) if e.kind() == ErrorKind::AddrInUse && attempt < ATTEMPTS => {
                if attempt == 1 {
                    println!("[Server] {addr} is busy, waiting for it to free up..");
                }
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
            Err(e) => return Err(e),
        }
    }
    unreachable!("the last attempt returns")
}
