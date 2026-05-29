use std::sync::Arc;

use protocol::packets::PacketRegistry;
use tokio::net::TcpListener;

mod codec;
mod entity;
mod protocol;
pub mod random;
pub mod world;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO:
    // ip port separated for config
    // check if the port is already binded
    let listener = TcpListener::bind("0.0.0.0:25565").await?;
    println!("Minecraft Server started at 0.0.0.0:25565");
    let packet_registry = Arc::new(PacketRegistry::new());

    loop {
        let (socket, _) = listener.accept().await?;
        let registry = packet_registry.clone();

        tokio::spawn(async move {
            codec::process(socket, registry).await;
        });
    }
}
