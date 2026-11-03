use crate::{
    packets::{PacketIn, PacketOut},
    reader::Reader,
};

#[derive(Debug)]
pub struct KeepAlive {
    pub id: i32,
}
impl PacketIn for KeepAlive {
    const ID: i32 = 0x00;

    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self> {
        let mut reader = Reader::new(buf);
        let id = reader.read_varint();
        Ok(Self { id })
    }
}
impl PacketOut for KeepAlive {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x00);
        writer.write_varint(self.id);
        Ok(())
    }
}
