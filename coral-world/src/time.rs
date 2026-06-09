use coral_protocol::packets::PacketOut;

#[derive(Debug)]
pub struct TimeUpdate {
    pub world_age: i64,
    pub time_of_day: i64,
}
impl PacketOut for TimeUpdate {
    fn encode(&self, writer: &mut coral_protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x03);
        writer.write_long(self.world_age);
        writer.write_long(self.time_of_day);
        Ok(())
    }
}
