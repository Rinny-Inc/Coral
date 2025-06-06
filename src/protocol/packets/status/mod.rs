use crate::protocol::packets::Packet;

pub struct Start;
pub struct Ping;
pub struct Done;
pub struct Pong {
    time: i64
}

// TODO
/*impl Packet for Ping {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
        where
            Self: Sized {
        
    }

    fn encode(&self, _writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        Ok(())
    }
}*/