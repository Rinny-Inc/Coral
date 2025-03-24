use tokio::net::TcpListener;

mod codec;
mod protocol;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:25565").await?;

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            codec::process(socket).await;
        });
    }
}