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

#[derive(Debug)]
pub struct SetSlot {
    pub window_id: i8,
    pub slot: i16,
    pub item_id: i16,
    pub count: u8,
    pub metadata: i16,
}
impl PacketOut for SetSlot {
    fn encode(&self, writer: &mut crate::protocol::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x2F);
        writer.write_byte(self.window_id as u8);
        writer.write_i16(self.slot);
        writer.write_i16(self.item_id);
        if self.item_id != -1 {
            writer.write_byte(self.count);
            writer.write_i16(self.metadata);
            writer.write_byte(0);
        }
        Ok(())
    }
}

pub struct Slot {
    pub item_id: i16,
    pub count: u8,
    pub metadata: i16,
}
pub struct Inventory {
    pub slots: [Option<Slot>; 44],
}
impl Inventory {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
        }
    }

    pub fn add_item_get_slot(&mut self, item_id: i16, count: u8, metadata: i16) -> Option<i16> {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if let Some(s) = slot
                && s.item_id == item_id
                && s.metadata == metadata
                && s.count < 64
            {
                s.count += count.min(64 - s.count);
                let packet_slot = Self::internal_to_packet(i);
                println!(
                    "DEBUG: stacked item {} in internal slot {}; packet slot {}",
                    item_id, i, packet_slot
                );
                return Some(packet_slot);
            }
        }
        for (i, slot) in self.slots.iter_mut().enumerate().take(36) {
            if slot.is_none() {
                *slot = Some(Slot {
                    item_id,
                    count,
                    metadata,
                });
                let packet_slot = Self::internal_to_packet(i);
                println!(
                    "DEBUG: placed item {} in internal slot {}; packet slot {}",
                    item_id, i, packet_slot
                );
                return Some(packet_slot);
            }
        }
        None
    }

    fn internal_to_packet(index: usize) -> i16 {
        match index {
            0..=8 => (index + 36) as i16, // hotbar: internal 0-8 -> packet 36-44
            9..=35 => index as i16,       // inventory: internal 9-35 -> packet 9-35
            36..=39 => (index - 36 + 5) as i16, // armor: 5-8
            _ => index as i16,
        }
    }

    fn packet_to_internal(slot: i16) -> Option<usize> {
        match slot {
            0..=8 => Some(slot as usize),                  // hotbar
            9..=35 => Some(slot as usize),                 // inventory
            100..=103 => Some((slot - 100 + 36) as usize), // gear
            80..=83 => Some((slot - 80 + 40) as usize),    // craft
            _ => None,
        }
    }
}
