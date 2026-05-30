use std::sync::{Arc, atomic::AtomicU32};

use base64::{Engine, engine::general_purpose::STANDARD};
use protocol::packets::PacketRegistry;
use tokio::net::TcpListener;

mod codec;
mod protocol;
pub mod world;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO:
    // ip port separated for config
    // check if the port is already binded
    let listener = TcpListener::bind("0.0.0.0:25565").await?;
    println!("Minecraft Server started at 0.0.0.0:25565");
    let packet_registry = Arc::new(PacketRegistry::new());
    let online = Arc::new(AtomicU32::new(0));

    let server_icon = load_server_icon();
    match &server_icon {
        Some(_) => println!("Server icon loaded successfully"),
        None => println!("No server icon found or invalid size"),
    }
    let server_icon = Arc::new(server_icon);

    loop {
        let (socket, _) = listener.accept().await?;
        let registry = packet_registry.clone();
        let online = online.clone();
        let server_icon = server_icon.clone();

        tokio::spawn(async move {
            codec::process(socket, registry, online, server_icon).await;
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
