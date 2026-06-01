use std::{io::ErrorKind, sync::Arc};

use base64::{Engine, engine::general_purpose::STANDARD};
use protocol::packets::PacketRegistry;
use tokio::{net::TcpListener, sync::broadcast};

use crate::{
    protocol::encryption::generate_rsa_key,
    server::{player::Player, registry::PlayerRegistry},
};

mod codec;
pub mod config;
mod protocol;
pub mod server;
pub mod world;

pub type PositionUpdate = (uuid::Uuid, i32, f64, f64, f64, f32, f32, bool);
pub type JoinLeave = (Player, bool);

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

    let (chat_tx, _) = broadcast::channel::<String>(100);
    let chat_tx = Arc::new(chat_tx);

    let (join_tx, _) = broadcast::channel::<JoinLeave>(100);
    let join_tx = Arc::new(join_tx);

    let (pos_tx, _) = broadcast::channel::<PositionUpdate>(100);
    let pos_tx = Arc::new(pos_tx);

    let (private_key, public_key_der) = generate_rsa_key();
    let private_key = Arc::new(private_key);
    let public_key_der = Arc::new(public_key_der);

    loop {
        let (socket, _) = listener.accept().await?;
        let registry = packet_registry.clone();
        let server_icon = server_icon.clone();
        let config = config.clone();
        let chat_tx = chat_tx.clone();
        let join_tx = join_tx.clone();
        let pos_tx = pos_tx.clone();
        let player_registry = player_registry.clone();
        let private_key = private_key.clone();
        let public_key_der = public_key_der.clone();

        tokio::spawn(async move {
            codec::process(
                socket,
                registry,
                server_icon,
                config,
                chat_tx,
                join_tx,
                pos_tx,
                player_registry,
                private_key,
                public_key_der,
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
