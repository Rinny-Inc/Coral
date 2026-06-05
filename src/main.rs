use std::{io::ErrorKind, sync::Arc};

use base64::{Engine, engine::general_purpose::STANDARD};
use protocol::packets::PacketRegistry;
use rsa::RsaPrivateKey;
use tokio::{
    net::TcpListener,
    sync::{RwLock, broadcast},
};
use uuid::Uuid;

use crate::{
    command::{CommandDispatcher, version_command},
    config::Config,
    protocol::encryption::generate_rsa_key,
    server::{ops::OpsFile, player::Player, registry::PlayerRegistry, whitelist::WhitelistFile},
    world::blocks::WorldBlocks,
};

mod codec;
pub mod command;
pub mod config;
mod protocol;
pub mod server;
pub mod world;

pub type PositionUpdate = (uuid::Uuid, i32, f64, f64, f64, f32, f32, bool);
pub type JoinLeave = (Player, bool);
pub type GamemodeUpdate = (Uuid, u8);
pub type PingUpdate = (Uuid, u32);
pub type BlockUpdate = (i32, i32, i32, i32, u8);
pub type AnimationUpdate = (i32, u8);
pub type MetadataUpdate = (i32, u8);
pub type DamageEvent = (Uuid, f32, i32, f32, i32);
pub type ItemDrop = (i32, f64, f64, f64);

#[derive(Clone)]
pub struct ServerContext {
    packet_registry: Arc<PacketRegistry>,
    player_registry: Arc<PlayerRegistry>,
    server_icon: Arc<Option<String>>,
    config: Arc<Config>,
    dispatcher: Arc<CommandDispatcher>,
    chat_tx: Arc<broadcast::Sender<String>>,
    join_tx: Arc<broadcast::Sender<JoinLeave>>,
    pos_tx: Arc<broadcast::Sender<PositionUpdate>>,
    gm_tx: Arc<broadcast::Sender<GamemodeUpdate>>,
    ping_tx: Arc<broadcast::Sender<PingUpdate>>,
    block_tx: Arc<broadcast::Sender<BlockUpdate>>,
    anim_tx: Arc<broadcast::Sender<AnimationUpdate>>,
    meta_tx: Arc<broadcast::Sender<MetadataUpdate>>,
    dmg_tx: Arc<broadcast::Sender<DamageEvent>>,
    item_tx: Arc<broadcast::Sender<ItemDrop>>,
    world_blocks: Arc<WorldBlocks>,
    private_key: Arc<RsaPrivateKey>,
    public_key_der: Arc<Vec<u8>>,
    ops: Arc<RwLock<OpsFile>>,
    whitelist: Arc<RwLock<WhitelistFile>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(config::Config::load());
    println!(
        "Loaded config: online_mode={}, port={}",
        config.server.online_mode, config.server.port
    );

    let addr = format!("0.0.0.0:{}", config.server.port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => {
            println!("Minecraft Server started at {}", addr);
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
    let server_icon = load_server_icon();
    match &server_icon {
        Some(_) => println!("Server icon loaded successfully"),
        None => println!("No server icon found or invalid size"),
    }

    let dispatcher = Arc::new(CommandDispatcher::new());
    dispatcher.register(version_command()).await;

    let (private_key, public_key_der) = generate_rsa_key();

    let ops = Arc::new(RwLock::new(OpsFile::load()));
    println!("Loaded {} opped players!", ops.read().await.entries.len());

    let whitelist = Arc::new(RwLock::new(WhitelistFile::load()));
    println!(
        "Loaded {} whitelisted players!",
        whitelist.read().await.entries.len()
    );

    let ctx = ServerContext {
        packet_registry: Arc::new(PacketRegistry::new()),
        server_icon: Arc::new(server_icon),
        config,
        dispatcher,
        chat_tx: Arc::new(broadcast::channel::<String>(50).0),
        join_tx: Arc::new(broadcast::channel::<JoinLeave>(16).0),
        pos_tx: Arc::new(broadcast::channel::<PositionUpdate>(100).0),
        gm_tx: Arc::new(broadcast::channel::<GamemodeUpdate>(16).0),
        ping_tx: Arc::new(broadcast::channel::<PingUpdate>(16).0),
        block_tx: Arc::new(broadcast::channel::<BlockUpdate>(100).0),
        anim_tx: Arc::new(broadcast::channel::<AnimationUpdate>(100).0),
        meta_tx: Arc::new(broadcast::channel::<MetadataUpdate>(100).0),
        dmg_tx: Arc::new(broadcast::channel::<DamageEvent>(100).0),
        item_tx: Arc::new(broadcast::channel::<ItemDrop>(1000).0),
        world_blocks: Arc::new(WorldBlocks::new()),
        player_registry: Arc::new(PlayerRegistry::new()),
        private_key: Arc::new(private_key),
        public_key_der: Arc::new(public_key_der),
        ops,
        whitelist,
    };

    loop {
        let (socket, _) = listener.accept().await?;
        let ctx = ctx.clone();

        tokio::spawn(async move {
            codec::process(socket, ctx).await;
        });
    }
}

fn load_server_icon() -> Option<String> {
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
