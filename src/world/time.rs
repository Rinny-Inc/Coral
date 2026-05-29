use std::io::Error;

use crate::protocol::packets::Packet;

#[derive(Debug)]
pub struct TimeUpdate {
    pub world_age: i64,
    pub time_of_day: i64,
}
impl Packet for TimeUpdate {
    fn decode(_buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        Err(Error::other("Unexpected Call!"))
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x03);
        writer.write_long(self.world_age);
        writer.write_long(self.time_of_day);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
