use std::{
    collections::{HashMap, VecDeque},
    io::ErrorKind,
    path::PathBuf,
    sync::{Arc, atomic::AtomicI64},
    time::Instant,
};

use base64::{Engine, engine::general_purpose::STANDARD};
use coral_protocol::packets::{
    PacketRegistry,
    play::{
        entity::{EntityAnimationType, TileEntity},
        game::EntityStatusType,
        movement::MovementBroadcast,
    },
};
use coral_types::{
    BedUpdate, BlockUpdate, BreakAnimation, ChestAnimation, DamageEvent, DespawnEntity,
    EntityVelocityUpdate, EquipmentUpdate, GamemodeUpdate, ItemDrop, ItemInfo, ItemPickup,
    KickRequest, MetadataUpdate, ParticleEffect, PingUpdate, PrivateMessage, ProjectileMove,
    SignUpdate, SoundEffect, SplashEffect, TeleportRequest, TicksExt, TimeUpdate, XpOrbMove,
    XpOrbSpawn, XpPickup,
};
use rsa::RsaPrivateKey;
use tokio::{
    net::TcpListener,
    sync::{
        RwLock,
        broadcast::{Sender, channel},
    },
};

use coral_command::{
    CommandDispatcher,
    list::{self, usage::ResourceMonitor},
};
use coral_config::Config;
use coral_protocol::encryption::generate_rsa_key;
use coral_server::{
    banlist::BanList,
    entity_tracker::EntityTracker,
    experience::XpOrb,
    items::ItemRegistry,
    ops::OpsFile,
    player::{Player, registry::PlayerRegistry},
    projectile::{Projectile, ProjectileKind},
    whitelist::WhitelistFile,
};
use coral_world::{
    blocks::{WorldBlocks, registry::BlockRegistry},
    generator::FlatWorldGenerator,
    level::{read_spawn_point, write_level_dat},
    weather::WeatherState,
};

mod codec;
mod fluid_sim;
mod tasks;

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
}

type JoinLeave = (Player, bool);
type AnimationUpdate = (i32, EntityAnimationType);
type EntityStatusUpdate = (i32, EntityStatusType);
type ProjectileSpawn = (i32, i32, ProjectileKind, f64, f64, f64, f64, f64, f64);

#[derive(Clone)]
pub struct Channels {
    chat_tx: Arc<Sender<String>>,
    join_tx: Arc<Sender<JoinLeave>>,
    pos_tx: Arc<Sender<MovementBroadcast>>,
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
    weather_tx: Arc<Sender<WeatherState>>,
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
    bed_tx: Arc<Sender<BedUpdate>>,
    wake_tx: Arc<Sender<()>>,
    private_msg_tx: Arc<Sender<PrivateMessage>>,
    teleport_rq_tx: Arc<Sender<TeleportRequest>>,
    kick_rq_tx: Arc<Sender<KickRequest>>,
    sign_update_tx: Arc<Sender<SignUpdate>>,
    velocity_broadcast_tx: Arc<Sender<EntityVelocityUpdate>>,
    chest_anim_tx: Arc<Sender<ChestAnimation>>,
    furnace_update_tx: Arc<Sender<(i32, i32, i32)>>,
}
impl Channels {
    pub fn new() -> Self {
        Self {
            chat_tx: Arc::new(channel::<String>(50).0),
            join_tx: Arc::new(channel::<JoinLeave>(16).0),
            pos_tx: Arc::new(channel::<MovementBroadcast>(100).0),
            gm_tx: Arc::new(channel::<GamemodeUpdate>(16).0),
            ping_tx: Arc::new(channel::<PingUpdate>(16).0),
            block_tx: Arc::new(channel::<BlockUpdate>(100).0),
            break_tx: Arc::new(channel::<BreakAnimation>(100).0),
            anim_tx: Arc::new(channel::<AnimationUpdate>(100).0),
            meta_tx: Arc::new(channel::<MetadataUpdate>(100).0),
            dmg_tx: Arc::new(channel::<DamageEvent>(100).0),
            item_tx: Arc::new(channel::<ItemDrop>(1000).0),
            despawn_tx: Arc::new(channel::<DespawnEntity>(50).0),
            pickup_tx: Arc::new(channel::<ItemPickup>(100).0),
            time_tx: Arc::new(channel::<TimeUpdate>(1).0),
            weather_tx: Arc::new(channel::<WeatherState>(1).0),
            tick_tx: Arc::new(channel(5).0),
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
            bed_tx: Arc::new(channel::<BedUpdate>(50).0),
            wake_tx: Arc::new(channel::<()>(4).0),
            private_msg_tx: Arc::new(channel::<PrivateMessage>(50).0),
            teleport_rq_tx: Arc::new(channel::<TeleportRequest>(5).0),
            kick_rq_tx: Arc::new(channel::<KickRequest>(5).0),
            sign_update_tx: Arc::new(channel::<SignUpdate>(5).0),
            velocity_broadcast_tx: Arc::new(channel::<EntityVelocityUpdate>(100).0),
            chest_anim_tx: Arc::new(channel::<ChestAnimation>(30).0),
            furnace_update_tx: Arc::new(channel::<(i32, i32, i32)>(100).0),
        }
    }
}

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

    let server_icon = load_server_icon_file()
        .inspect(|_| println!("Server icon loaded successfully"))
        .or_else(|| {
            println!("No server icon found or invalid size");
            None
        });

    let player_registry = Arc::new(PlayerRegistry::new());
    let channels = Channels::new();
    let ops = Arc::new(RwLock::new(OpsFile::load()));
    let whitelist = Arc::new(RwLock::new(WhitelistFile::load()));
    let world_dir = std::path::Path::new(&config.world.world_name);
    let spawn_point = read_spawn_point(world_dir)
        .await
        .unwrap_or((0.5, 5.0, 0.5, 0.0, 0.0));

    let world_blocks = Arc::new(WorldBlocks::new());
    let generator = Arc::new(FlatWorldGenerator::new());

    world_blocks.load(world_dir, &generator).await;

    if !world_dir.join("level.dat").exists() {
        write_level_dat(world_dir, "world");
    }

    let tile_entities = Arc::new(RwLock::new(HashMap::new()));

    tasks::spawn_furnace_task(tile_entities.clone(), channels.clone());

    if config.world.enable_auto_save {
        tasks::spawn_world_save_task(
            world_blocks.clone(),
            generator.clone(),
            world_dir.to_path_buf(),
            tile_entities.clone(),
            config.world.auto_save_interval,
        );
    }

    let spawn_point = Arc::new(RwLock::new(spawn_point));
    let world_dir = Arc::new(world_dir.to_path_buf());

    let dispatcher = Arc::new(CommandDispatcher::new());
    dispatcher.register(list::version::command()).await;
    dispatcher
        .register(list::player_list::command(player_registry.clone()))
        .await;
    dispatcher
        .register(list::gamemode::command(
            player_registry.clone(),
            channels.gm_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::kill::command(
            player_registry.clone(),
            channels.dmg_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::op::command(player_registry.clone(), ops.clone()))
        .await;
    dispatcher
        .register(list::deop::command(player_registry.clone(), ops.clone()))
        .await;
    dispatcher
        .register(list::whitelist::command(
            player_registry.clone(),
            whitelist.clone(),
        ))
        .await;
    dispatcher.register(list::say::command()).await;
    dispatcher
        .register(list::msg::command(
            player_registry.clone(),
            channels.private_msg_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::reply::command(
            player_registry.clone(),
            channels.private_msg_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::usage::command(resource_monitor.clone()))
        .await;
    dispatcher
        .register(
            list::setworldspawn::command(
                player_registry.clone(),
                spawn_point.clone(),
                world_dir.clone(),
            )
            .await,
        )
        .await;
    dispatcher
        .register(list::teleport::command(
            player_registry.clone(),
            channels.teleport_rq_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::kick::command(
            player_registry.clone(),
            channels.kick_rq_tx.clone(),
        ))
        .await;
    dispatcher
        .register(list::ping::command(player_registry.clone()))
        .await;

    let (private_key, public_key_der) = generate_rsa_key();

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
        world_blocks,
        world_time: Arc::new(AtomicI64::new(0)),
        generator,
        player_registry,
        private_key: Arc::new(private_key),
        public_key_der: Arc::new(public_key_der),
        ops,
        whitelist,
        banlist: Arc::new(RwLock::new(BanList::load())),
        spawn_point,
        world_dir: world_dir.clone(),
        xp_orbs: Arc::new(RwLock::new(Vec::new())),
        fluid_queue: Arc::new(RwLock::new(VecDeque::new())),
        tile_entities,
    };

    tasks::spawn_console_task(ctx.dispatcher.clone(), ctx.channels.chat_tx.clone());
    tasks::spawn_shutdown_task(
        ctx.channels.shutdown_tx.clone(),
        ctx.player_registry.clone(),
        ctx.world_blocks.clone(),
        world_dir.to_path_buf(),
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
