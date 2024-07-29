use tokio::net::{TcpListener, TcpStream};
use tokio_stream::StreamExt;
use bytes::{Buf, BufMut, BytesMut};
use tokio_util::codec::{Decoder, Encoder, Framed};
use futures::SinkExt;

struct MinecraftCodec;

impl Decoder for MinecraftCodec {
    type Item = Vec<u8>;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 1 {
            return Ok(None);
        }

        let length = src[0] as usize;
        if src.len() < length + 1 {
            return Ok(None);
        }

        src.advance(1);
        let data = src.split_to(length);
        Ok(Some(data.to_vec()))
    }
}

impl Encoder<Vec<u8>> for MinecraftCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: Vec<u8>, dst: &mut BytesMut) -> Result<(), Self::Error> {
        dst.put_u8(item.len() as u8);
        dst.put_slice(&item);
        Ok(())
    }
}

async fn process(socket: TcpStream) {
    let mut framed = Framed::new(socket, MinecraftCodec);

    while let Some(Ok(packet)) = framed.next().await {
        println!("Received packet: {:?}", packet);

        if packet[0] == 0x01 {
            let response = vec![0x01, 0x00, 0x00, 0x00, 0x00];
            if let Err(e) = framed.send(response).await {
                eprintln!("Error sending response: {:?}", e);
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:25565").await?;

    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            process(socket).await;
        });
    }
}