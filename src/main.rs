use std::sync::Arc;

use protocol::packets::PacketRegistry;
use tokio::net::TcpListener;

mod codec;
mod protocol;
mod entity;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:25565").await?;
    let packet_registry = Arc::new(PacketRegistry::new());

    loop {
        let (socket, _) = listener.accept().await?;
        let registry = packet_registry.clone();

        tokio::spawn(async move {
            codec::process(socket, registry).await;
        });
    }
}