use crate::protocol::{packets::Packet, reader::Reader};

#[derive(Debug)]
pub struct KeepAlive {
    pub id: i32,
}
impl Packet for KeepAlive {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf.to_vec());
        let _packet_id = reader.read_varint();
        let id = reader.read_varint();
        Ok(Self { id })
    }

    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x00);
        writer.write_varint(self.id);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
