use crate::{
    packets::{
        PacketIn, PacketOut,
        play::{block::BlockPosition, chat::builder::ChatBuilder},
    },
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
    pub window_type: WindowType,
    pub title: ChatBuilder,
    pub slot_count: u8,
    // todo i32 entity_id (only when riding horse)
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
        writer.write_string(self.window_type.id());
        writer.write_string(&self.title.to_json());
        writer.write_byte(self.slot_count);
        // writer.write_bool(false); // only when riding a horse
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

/// Result of [`Inventory::add_item`]
#[derive(Debug, Default, PartialEq, Eq)]
pub struct AddOutcome {
    /// `(packet_slot, internal_index)` for every slot changed
    pub touched: Vec<(i16, usize)>,
    /// How much of the stack did not fit
    pub leftover: u8,
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

    // FIXME: I think it doesnt behave as it should
    pub fn insert_itemstack(&mut self, mut stack: ItemStack) -> Option<ItemStack> {
        // first merge into existing matching stacks
        for existing in self.slots.iter_mut().take(36).flatten() {
            if existing.item_id == stack.item_id
                && existing.metadata == stack.metadata
                && existing.count < 64
            {
                let space = 64 - existing.count;
                let move_n = space.min(stack.count);
                existing.count += move_n;
                stack.count -= move_n;
                if stack.count == 0 {
                    return None;
                }
            }
        }
        // second place into empty space
        for slot in self.slots.iter_mut().take(36) {
            if slot.is_none() {
                *slot = Some(stack);
                return None;
            }
        }
        Some(stack) // didnt fit
    }

    pub fn add_item(&mut self, item_id: i16, count: u8, metadata: i16) -> AddOutcome {
        let mut remaining = count;
        let mut touched = Vec::new();

        // stack with existing
        for (i, slot) in self.slots.iter_mut().enumerate().take(36) {
            if remaining == 0 {
                break;
            }
            if let Some(s) = slot
                && s.item_id == item_id
                && s.metadata == metadata
                && s.count < 64
            {
                let moved = (64 - s.count).min(remaining);
                s.count += moved;
                remaining -= moved;
                touched.push((Self::internal_to_packet(i), i));
            }
        }
        // then empty slot
        for (i, slot) in self.slots.iter_mut().enumerate().take(36) {
            if remaining == 0 {
                break;
            }
            if slot.is_none() {
                let placed = remaining.min(64);
                *slot = Some(ItemStack {
                    item_id,
                    count: placed,
                    metadata,
                    durability: 0,
                });
                remaining -= placed;
                touched.push((Self::internal_to_packet(i), i));
            }
        }

        AddOutcome {
            touched,
            leftover: remaining,
        }
    }

    fn internal_to_packet(index: usize) -> i16 {
        match index {
            0..=8 => (index + 36) as i16, // hotbar: internal 0-8 -> packet 36-44
            9..=35 => index as i16,       // inventory: internal 9-35 -> packet 9-35
            36..=39 => (index - 36 + 5) as i16, // armor: 5-8
            _ => index as i16,
        }
    }

    pub const ARMOR_HELMET: usize = 36;
    pub const ARMOR_CHESTPLATE: usize = 37;
    pub const ARMOR_LEGGINGS: usize = 38;
    pub const ARMOR_BOOTS: usize = 39;
    pub const ARMOR_SLOTS: std::ops::RangeInclusive<usize> = 36..=39;

    pub fn packet_to_internal(slot: i16) -> Option<usize> {
        match slot {
            36..=44 => Some((slot - 36) as usize),
            9..=35 => Some(slot as usize),
            5..=8 => Some((slot - 5 + 36) as usize),
            _ => None,
        }
    }
}
impl Default for Inventory {
    fn default() -> Self {
        Self::new()
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

#[derive(Debug, Clone)]
pub enum WindowType {
    Chest {
        window_id: u8,
        pos: (i32, i32, i32),
    },
    Furnace {
        window_id: u8,
        pos: (i32, i32, i32),
    },
    // 2x2 is inventory; 3x3 is table
    Crafting {
        window_id: u8,
    },
    Enchanting {
        window_id: u8,
        pos: (i32, i32, i32),
    },
    Anvil {
        window_id: u8,
    },
    Brewing {
        window_id: u8,
        pos: (i32, i32, i32),
    },
    Dispenser {
        window_id: u8,
        pos: (i32, i32, i32),
        drop: bool,
    },
    Hopper {
        window_id: u8,
        pos: (i32, i32, i32),
    },
    Beacon {
        window_id: u8,
        pos: (i32, i32, i32),
    },
}
impl WindowType {
    pub fn window_id(&self) -> u8 {
        match self {
            WindowType::Chest { window_id, .. } => *window_id,
            WindowType::Furnace { window_id, .. } => *window_id,
            WindowType::Crafting { window_id } => *window_id,
            WindowType::Enchanting { window_id, .. } => *window_id,
            WindowType::Anvil { window_id } => *window_id,
            WindowType::Brewing { window_id, .. } => *window_id,
            WindowType::Dispenser { window_id, .. } => *window_id,
            WindowType::Hopper { window_id, .. } => *window_id,
            WindowType::Beacon { window_id, .. } => *window_id,
        }
    }
    pub fn id(&self) -> &'static str {
        match self {
            WindowType::Chest { .. } => "minecraft:chest",
            WindowType::Furnace { .. } => "minecraft:furnace",
            WindowType::Crafting { .. } => "minecraft:crafting_table",
            WindowType::Enchanting { .. } => "minecraft:enchanting_table",
            WindowType::Anvil { .. } => "minecraft:anvil",
            WindowType::Brewing { .. } => "minecraft:brewing_stand",
            WindowType::Dispenser { drop, .. } => {
                if *drop {
                    "minecraft:dropper"
                } else {
                    "minecraft:dispenser"
                }
            }
            WindowType::Hopper { .. } => "minecraft:hopper",
            WindowType::Beacon { .. } => "minecraft:beacon",
        }
    }
}

#[derive(Debug)]
pub struct SignEditorOpen {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl PacketOut for SignEditorOpen {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x36);
        let pos = BlockPosition::new(self.x, self.y as u8, self.z);
        writer.write_block_position(pos);
        Ok(())
    }
}

#[derive(Debug)]
pub struct WindowProperty {
    pub window_id: u8,
    pub property: i16,
    pub value: i16,
}
impl PacketOut for WindowProperty {
    fn encode(&self, writer: &mut crate::writer::Writer) -> std::io::Result<()> {
        writer.write_varint(0x31);
        writer.write_byte(self.window_id);
        writer.write_i16(self.property);
        writer.write_i16(self.value);
        Ok(())
    }
}

#[cfg(test)]
mod armor_slot_tests {
    use super::*;

    #[test]
    fn packet_armor_slots_map_to_the_named_constants() {
        assert_eq!(
            Inventory::packet_to_internal(5),
            Some(Inventory::ARMOR_HELMET)
        );
        assert_eq!(
            Inventory::packet_to_internal(6),
            Some(Inventory::ARMOR_CHESTPLATE)
        );
        assert_eq!(
            Inventory::packet_to_internal(7),
            Some(Inventory::ARMOR_LEGGINGS)
        );
        assert_eq!(
            Inventory::packet_to_internal(8),
            Some(Inventory::ARMOR_BOOTS)
        );
    }

    #[test]
    fn the_old_range_was_the_hotbar_not_the_armor() {
        for packet_slot in 41..=44i16 {
            let idx = Inventory::packet_to_internal(packet_slot).unwrap();
            assert!((5..=8).contains(&idx), "packet {packet_slot} -> {idx}");
            assert!(
                !Inventory::ARMOR_SLOTS.contains(&idx),
                "packet {packet_slot} is hotbar, not armor"
            );
        }
        // and no armor click ever landed in the old range
        for packet_slot in 5..=8i16 {
            let idx = Inventory::packet_to_internal(packet_slot).unwrap();
            assert!(
                !(5..=8).contains(&idx),
                "armor packet {packet_slot} -> {idx}"
            );
        }
    }

    #[test]
    fn the_armor_range_matches_the_constants() {
        assert!(Inventory::ARMOR_SLOTS.contains(&Inventory::ARMOR_HELMET));
        assert!(Inventory::ARMOR_SLOTS.contains(&Inventory::ARMOR_BOOTS));
        assert!(!Inventory::ARMOR_SLOTS.contains(&35));
        assert!(!Inventory::ARMOR_SLOTS.contains(&40));
    }

    #[test]
    fn clicking_an_armor_slot_is_detected() {
        for packet_slot in 5..=8i16 {
            let idx = Inventory::packet_to_internal(packet_slot).unwrap();
            assert!(Inventory::ARMOR_SLOTS.contains(&idx));
        }
        // and a hotbar click is not
        let hotbar = Inventory::packet_to_internal(36).unwrap();
        assert!(!Inventory::ARMOR_SLOTS.contains(&hotbar));
    }
}

#[cfg(test)]
mod add_item_tests {
    use super::*;

    fn stack(id: i16, count: u8) -> ItemStack {
        ItemStack {
            item_id: id,
            count,
            metadata: 0,
            durability: 0,
        }
    }

    #[test]
    fn the_remainder_spills_into_the_next_slot_instead_of_vanishing() {
        let mut inv = Inventory::new();
        inv.slots[0] = Some(stack(1, 60));

        let out = inv.add_item(1, 10, 0);
        assert_eq!(out.leftover, 0, "nothing may be destroyed");
        assert_eq!(inv.slots[0].as_ref().unwrap().count, 64);
        let spilled: u8 = inv.slots[1..].iter().flatten().map(|s| s.count).sum();
        assert_eq!(spilled, 6);
        assert_eq!(out.touched.len(), 2);
    }

    #[test]
    fn pickups_never_touch_armor_or_crafting_slots() {
        let mut inv = Inventory::new();
        inv.slots[Inventory::ARMOR_HELMET] = Some(stack(1, 1));
        inv.slots[40] = Some(stack(1, 1)); // crafting

        let out = inv.add_item(1, 5, 0);
        assert_eq!(
            inv.slots[Inventory::ARMOR_HELMET].as_ref().unwrap().count,
            1
        );
        assert_eq!(inv.slots[40].as_ref().unwrap().count, 1);
        assert!(out.touched.iter().all(|(_, i)| *i < 36));
        assert_eq!(out.leftover, 0);
    }

    #[test]
    fn a_full_inventory_reports_the_whole_stack_as_leftover() {
        let mut inv = Inventory::new();
        for i in 0..36 {
            inv.slots[i] = Some(stack(2, 64));
        }
        let out = inv.add_item(1, 5, 0);
        assert_eq!(out.leftover, 5);
        assert!(out.touched.is_empty(), "nothing should have changed");
    }

    #[test]
    fn a_partly_full_inventory_takes_what_fits() {
        let mut inv = Inventory::new();
        for i in 0..36 {
            inv.slots[i] = Some(stack(1, 64));
        }
        inv.slots[35] = Some(stack(1, 60));

        let out = inv.add_item(1, 10, 0);
        assert_eq!(out.leftover, 6);
        assert_eq!(inv.slots[35].as_ref().unwrap().count, 64);
    }

    #[test]
    fn metadata_must_match_to_stack() {
        let mut inv = Inventory::new();
        inv.slots[0] = Some(ItemStack {
            item_id: 35,
            count: 10,
            metadata: 1,
            durability: 0,
        });
        let out = inv.add_item(35, 5, 14);
        assert_eq!(
            inv.slots[0].as_ref().unwrap().count,
            10,
            "different metadata merged"
        );
        assert_eq!(out.leftover, 0);
        assert_eq!(inv.slots[1].as_ref().unwrap().metadata, 14);
    }

    #[test]
    fn an_empty_inventory_takes_a_whole_stack() {
        let mut inv = Inventory::new();
        let out = inv.add_item(1, 64, 0);
        assert_eq!(out.leftover, 0);
        assert_eq!(out.touched.len(), 1);
        assert_eq!(inv.slots[0].as_ref().unwrap().count, 64);
    }
}
