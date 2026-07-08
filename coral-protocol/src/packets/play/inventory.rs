use crate::{
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
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
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
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x2D);
        writer.write_byte(self.window_id);
        writer.write_string(&self.window_type);
        writer.write_string(&self.title);
        writer.write_byte(self.slot_count);
        writer.write_bool(false);
        Ok(())
    }
}

impl PacketIn for ConfirmTransaction {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let window_id = reader.read_byte();
        let action_number = reader.read_i16();
        let accepted = reader.read_bool();
        Ok(ConfirmTransaction {
            window_id,
            action_number,
            accepted,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
impl PacketOut for ConfirmTransaction {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
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
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
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

#[derive(Debug, Clone)]
pub struct ItemStack {
    pub item_id: i16,
    pub count: u8,
    pub metadata: i16,
    pub durability: i16,
}
pub struct Inventory {
    pub slots: [Option<ItemStack>; 44],
}
impl Inventory {
    pub fn new() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
        }
    }

    pub fn add_item_get_slot(
        &mut self,
        item_id: i16,
        count: u8,
        metadata: i16,
    ) -> Option<(i16, usize)> {
        // stack with existing
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if let Some(s) = slot
                && s.item_id == item_id
                && s.metadata == metadata
                && s.count < 64
            {
                s.count += count.min(64 - s.count);
                return Some((Self::internal_to_packet(i), i));
            }
        }
        // empty slot
        for (i, slot) in self.slots.iter_mut().enumerate().take(36) {
            if slot.is_none() {
                *slot = Some(ItemStack {
                    item_id,
                    count,
                    metadata,
                    durability: 0,
                });
                return Some((Self::internal_to_packet(i), i));
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

    pub fn packet_to_internal(slot: i16) -> Option<usize> {
        match slot {
            36..=44 => Some((slot - 36) as usize),
            9..=35 => Some(slot as usize),
            5..=8 => Some((slot - 5 + 36) as usize),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct WindowItems {
    pub window_id: u8,
    pub slots: Vec<(i16, u8, i16)>,
}
impl PacketOut for WindowItems {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x30);
        writer.write_byte(self.window_id);
        writer.write_i16(self.slots.len() as i16);
        for (item_id, count, metadata) in &self.slots {
            writer.write_i16(*item_id);
            if *item_id != -1 {
                writer.write_byte(*count);
                writer.write_i16(*metadata);
                writer.write_byte(0);
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CreativeInventoryAction {
    pub slot: i16,
    pub item_id: i16,
    pub item_count: u8,
    pub item_damage: i16,
}
impl PacketIn for CreativeInventoryAction {
    fn decode(buf: &mut bytes::Bytes) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        let mut reader = Reader::new(buf);
        let slot = reader.read_i16();
        let item_id = reader.read_i16();
        let (item_count, item_damage) = if item_id != -1 {
            (reader.read_byte(), reader.read_i16())
        } else {
            (0, 0)
        };
        Ok(CreativeInventoryAction {
            slot,
            item_id,
            item_count,
            item_damage,
        })
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
