use std::io::Error;

use crate::protocol::packets::Packet;

#[derive(Debug)]
pub struct ChangeGameState {
    // 0 = invalid bed
    // 1 = end raining
    // 2 = begin raining
    // 3 = change gamemode
    // 4 = enter credits
    // 5 = demo message
    // 6 = arrow hit player
    // 7 = fade value
    // 8 = fade time
    // 10 = play mob appearance
    pub reason: u8,
    pub value: f32,
}
impl ChangeGameState {
    pub fn set_gamemode(gamemode: u8) -> Self {
        Self {
            reason: 3,
            value: gamemode as f32,
        }
    }
}
impl Packet for ChangeGameState {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x2B);
        writer.write_byte(self.reason);
        writer.write_f32(self.value);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
