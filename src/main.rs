use std::{
    io::ErrorKind,
    sync::{Arc, atomic::AtomicU32},
};

use base64::{Engine, engine::general_purpose::STANDARD};
use protocol::packets::PacketRegistry;
use tokio::{net::TcpListener, sync::broadcast};

mod codec;
pub mod config;
mod protocol;
pub mod server;
pub mod world;

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
    let online = Arc::new(AtomicU32::new(0));

    let server_icon = load_server_icon();
    match &server_icon {
        Some(_) => println!("Server icon loaded successfully"),
        None => println!("No server icon found or invalid size"),
    }
    let server_icon = Arc::new(server_icon);

    let (chat_tx, _) = broadcast::channel::<String>(100); // TODO: probably more?
    let chat_tx = Arc::new(chat_tx);

    loop {
        let (socket, _) = listener.accept().await?;
        let registry = packet_registry.clone();
        let online = online.clone();
        let server_icon = server_icon.clone();
        let config = config.clone();
        let chat_tx = chat_tx.clone();

        tokio::spawn(async move {
            codec::process(socket, registry, online, server_icon, config, chat_tx).await;
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
