use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicI64},
    time::Instant,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use coral::{Channels, register_commands, world_path};
use coral_protocol::packets::{PacketRegistry, play::entity::TileEntity};
use coral_types::{BreakAnimation, EquipmentUpdate, ItemInfo, ItemPickup, SoundEffect};
use rsa::RsaPrivateKey;
use tokio::{net::TcpListener, sync::RwLock};

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
    blocks::{WorldBlocks, registry::BlockRegistry},
    generator::FlatWorldGenerator,
    level::{read_spawn_point, write_level_dat},
};

mod codec;
mod fluid_sim;
mod tasks;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let resource_monitor = Arc::new(ResourceMonitor::new());
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

    let cwd = std::env::current_dir()?;

    let server_icon = load_server_icon_file(&cwd)
        .inspect(|_| println!("Server icon loaded successfully"))
        .or_else(|| {
            println!("No server icon found or invalid size");
            None
        });

    let world_path = world_path(&config.world.world_name)?;
    let world_dir = cwd.join(&config.world.world_name);

    if !world_dir.join("level.dat").exists() {
        write_level_dat(world_path, &config.world.world_name);
    }

    let spawn_point = read_spawn_point(world_path)
        .await
        .unwrap_or((0.5, 5.0, 0.5, 0.0, 0.0));

    let world_blocks = Arc::new(WorldBlocks::new());
    let generator = Arc::new(FlatWorldGenerator::new());
    world_blocks.load(world_path, &generator).await;

    let (private_key, public_key_der) = generate_rsa_key();

    let ctx = ServerContext::new(
        server_icon,
        config.clone(),
        world_blocks,
        generator,
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
        &ctx.world_time,
    )
    .await;

    tasks::spawn_furnace_task(
        ctx.tile_entities.clone(),
        ctx.world_blocks.clone(),
        ctx.generator.clone(),
        ctx.channels.clone(),
    );

    if config.world.enable_auto_save {
        tasks::spawn_world_save_task(
            ctx.world_blocks.clone(),
            ctx.generator.clone(),
            ctx.world_dir.to_path_buf(),
            ctx.tile_entities.clone(),
            config.world.auto_save_interval,
        );
    }

    tasks::spawn_console_task(ctx.dispatcher.clone(), ctx.channels.chat_tx.clone());
    tasks::spawn_shutdown_task(
        ctx.channels.shutdown_tx.clone(),
        ctx.player_registry.clone(),
        ctx.world_blocks.clone(),
        ctx.world_dir.to_path_buf(),
        ctx.tile_entities.clone(),
        ctx.generator.clone(),
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
        ctx.world_blocks.clone(),
        ctx.generator.clone(),
        ctx.player_registry.clone(),
        ctx.channels.clone(),
    );

    tasks::spawn_chunk_cache_cleanup_task(ctx.world_blocks.clone());

    tasks::spawn_xp_orb_task(
        ctx.xp_orbs.clone(),
        ctx.world_blocks.clone(),
        ctx.generator.clone(),
        ctx.player_registry.clone(),
        ctx.channels.clone(),
    );

    fluid_sim::spawn_fluid_task(
        ctx.fluid_queue.clone(),
        ctx.world_blocks.clone(),
        ctx.generator.clone(),
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
    world_blocks: Arc<WorldBlocks>,
    world_time: Arc<AtomicI64>,
    generator: Arc<FlatWorldGenerator>,
    private_key: Arc<RsaPrivateKey>,
    public_key_der: Arc<Vec<u8>>,
    ops: Arc<RwLock<OpsFile>>,
    whitelist: Arc<RwLock<WhitelistFile>>,
    banlist: Arc<RwLock<BanList>>,
    spawn_point: Arc<RwLock<(f64, f64, f64, f32, f32)>>,
    world_dir: Arc<PathBuf>,
    xp_orbs: Arc<RwLock<Vec<XpOrb>>>,
    fluid_queue: Arc<RwLock<VecDeque<(i32, i32, i32)>>>,
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
        world_blocks: Arc<WorldBlocks>,
        generator: Arc<FlatWorldGenerator>,
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
            world_blocks,
            world_time: Arc::new(AtomicI64::new(0)),
            generator,
            player_registry: Arc::new(PlayerRegistry::new()),
            private_key: Arc::new(private_key),
            public_key_der: Arc::new(public_key_der),
            ops: Arc::new(RwLock::new(OpsFile::load())),
            whitelist: Arc::new(RwLock::new(WhitelistFile::load())),
            banlist: Arc::new(RwLock::new(BanList::load())),
            spawn_point: Arc::new(RwLock::new(spawn_point)),
            world_dir: Arc::new(world_dir),
            xp_orbs: Arc::new(RwLock::new(Vec::new())),
            fluid_queue: Arc::new(RwLock::new(VecDeque::new())),
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
