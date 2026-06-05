use std::{io::ErrorKind, sync::Arc};

use base64::{Engine, engine::general_purpose::STANDARD};
use protocol::packets::PacketRegistry;
use tokio::{
    net::TcpListener,
    sync::{RwLock, broadcast},
};
use uuid::Uuid;

use crate::{
    command::{CommandDispatcher, version_command},
    protocol::encryption::generate_rsa_key,
    server::{ops::OpsFile, player::Player, registry::PlayerRegistry},
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
    let packet_registry = Arc::new(PacketRegistry::new());
    let player_registry = Arc::new(PlayerRegistry::new());

    let server_icon = load_server_icon();
    match &server_icon {
        Some(_) => println!("Server icon loaded successfully"),
        None => println!("No server icon found or invalid size"),
    }
    let server_icon = Arc::new(server_icon);

    let (chat_tx, _) = broadcast::channel::<String>(50); // We dont need to store that much chat messages
    let chat_tx = Arc::new(chat_tx);

    let (join_tx, _) = broadcast::channel::<JoinLeave>(1); // We dont need to store 100 join and leave action
    let join_tx = Arc::new(join_tx);

    let (pos_tx, _) = broadcast::channel::<PositionUpdate>(100);
    let pos_tx = Arc::new(pos_tx);

    let (gm_tx, _) = broadcast::channel::<GamemodeUpdate>(1);
    let gm_tx = Arc::new(gm_tx);

    let (ping_tx, _) = broadcast::channel::<PingUpdate>(1);
    let ping_tx = Arc::new(ping_tx);

    let (block_tx, _) = broadcast::channel::<BlockUpdate>(10);
    let block_tx = Arc::new(block_tx);

    let (anim_tx, _) = broadcast::channel::<AnimationUpdate>(100);
    let anim_tx = Arc::new(anim_tx);

    let (meta_tx, _) = broadcast::channel::<MetadataUpdate>(100);
    let meta_tx = Arc::new(meta_tx);

    let (dmg_tx, _) = broadcast::channel::<DamageEvent>(100);
    let dmg_tx = Arc::new(dmg_tx);

    let world_blocks = Arc::new(WorldBlocks::new());
    let command_dispatcher = Arc::new(CommandDispatcher::new());
    command_dispatcher.register(version_command()).await;

    let (private_key, public_key_der) = generate_rsa_key();
    let private_key = Arc::new(private_key);
    let public_key_der = Arc::new(public_key_der);

    let ops = Arc::new(RwLock::new(OpsFile::load()));
    println!("Loaded {} opped players!", ops.read().await.entries.len());

    loop {
        let (socket, _) = listener.accept().await?;
        let registry = packet_registry.clone();
        let server_icon = server_icon.clone();
        let config = config.clone();
        let dispatcher = command_dispatcher.clone();

        let chat_tx = chat_tx.clone();
        let join_tx = join_tx.clone();
        let pos_tx = pos_tx.clone();
        let gm_tx = gm_tx.clone();
        let ping_tx = ping_tx.clone();
        let block_tx = block_tx.clone();
        let world_blocks = world_blocks.clone();
        let anim_tx = anim_tx.clone();
        let meta_tx = meta_tx.clone();
        let dmg_tx = dmg_tx.clone();

        let player_registry = player_registry.clone();
        let private_key = private_key.clone();
        let public_key_der = public_key_der.clone();

        let ops = ops.clone();

        tokio::spawn(async move {
            codec::process(
                socket,
                registry,
                server_icon,
                config,
                dispatcher,
                chat_tx,
                join_tx,
                pos_tx,
                gm_tx,
                ping_tx,
                block_tx,
                anim_tx,
                meta_tx,
                dmg_tx,
                world_blocks,
                player_registry,
                private_key,
                public_key_der,
                ops,
            )
            .await;
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
