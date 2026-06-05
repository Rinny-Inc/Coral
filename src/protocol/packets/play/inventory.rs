use crate::protocol::{
    packets::{PacketIn, PacketOut},
    reader::Reader,
};

#[derive(Debug)]
pub struct CloseWindow {
    pub window_id: u8,
}

#[derive(Debug)]
pub struct ClickWindow {
    pub window_id: u8,
    pub slot: i16,
    pub button: u8,
    pub action_number: i16,
    pub mode: u8,
    pub clicked_item_id: i16,
}

#[derive(Debug)]
pub struct OpenWindow {
    pub window_id: u8,
    pub window_type: String,
    pub title: String,
    pub slot_count: u8,
}

#[derive(Debug)]
pub struct ConfirmTransaction {
    pub window_id: u8,
    pub action_number: i16,
    pub accepted: bool,
}

impl PacketIn for CloseWindow {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let window_id = reader.read_byte();
        Ok(CloseWindow { window_id })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for CloseWindow {
    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x2E);
        writer.write_byte(self.window_id);
        Ok(())
    }
}

impl PacketIn for ClickWindow {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let window_id = reader.read_byte();
        let slot = reader.read_i16();
        let button = reader.read_byte();
        let action_number = reader.read_i16();
        let mode = reader.read_byte();
        let clicked_item_id = reader.read_i16();
        Ok(ClickWindow {
            window_id,
            slot,
            button,
            action_number,
            mode,
            clicked_item_id,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl PacketOut for OpenWindow {
    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x2D);
        writer.write_byte(self.window_id);
        writer.write_string(&self.window_type);
        writer.write_string(&self.title);
        writer.write_byte(self.slot_count);
        writer.write_bool(false);
        Ok(())
    }
}

impl PacketOut for ConfirmTransaction {
    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x32);
        writer.write_byte(self.window_id);
        writer.write_i16(self.action_number);
        writer.write_bool(self.accepted);
        Ok(())
    }
}
