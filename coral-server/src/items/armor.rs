use coral_types::ToolMaterial;
use coral_types::ids::items;

use crate::items::{Item, ItemRegistry};

#[derive(Debug, Clone, PartialEq)]
pub enum ArmorSlot {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

pub struct Armor {
    id: i16,
    slot: ArmorSlot,
    pub material: ToolMaterial,
    defense: i32,
    durability: i16,
}

impl Armor {
    pub fn equipment_slot(&self) -> u8 {
        match self.slot {
            ArmorSlot::Boots => 1,
            ArmorSlot::Leggings => 2,
            ArmorSlot::Chestplate => 3,
            ArmorSlot::Helmet => 4,
        }
    }
}
impl Item for Armor {
    fn id(&self) -> i16 {
        self.id
    }
    fn max_stack_size(&self) -> u8 {
        1
    }
    fn max_durability(&self) -> Option<i16> {
        Some(self.durability)
    }
    fn armor_defense(&self) -> Option<i32> {
        Some(self.defense)
    }
    fn armor_slot(&self) -> Option<ArmorSlot> {
        Some(self.slot.clone())
    }
}

// todo: change wood and stone for leather and chainmail
pub fn all() -> Vec<Armor> {
    use ArmorSlot::*;
    use ToolMaterial::*;

    vec![
        Armor {
            id: items::LEATHER_HELMET,
            slot: Helmet,
            material: Wood,
            defense: 1,
            durability: 55,
        },
        Armor {
            id: items::LEATHER_CHESTPLATE,
            slot: Chestplate,
            material: Wood,
            defense: 3,
            durability: 80,
        },
        Armor {
            id: items::LEATHER_LEGGINGS,
            slot: Leggings,
            material: Wood,
            defense: 2,
            durability: 75,
        },
        Armor {
            id: items::LEATHER_BOOTS,
            slot: Boots,
            material: Wood,
            defense: 1,
            durability: 65,
        },
        Armor {
            id: items::CHAINMAIL_HELMET,
            slot: Helmet,
            material: Stone,
            defense: 2,
            durability: 165,
        },
        Armor {
            id: items::CHAINMAIL_CHESTPLATE,
            slot: Chestplate,
            material: Stone,
            defense: 5,
            durability: 240,
        },
        Armor {
            id: items::CHAINMAIL_LEGGINGS,
            slot: Leggings,
            material: Stone,
            defense: 4,
            durability: 225,
        },
        Armor {
            id: items::CHAINMAIL_BOOTS,
            slot: Boots,
            material: Stone,
            defense: 1,
            durability: 195,
        },
        Armor {
            id: items::IRON_HELMET,
            slot: Helmet,
            material: Iron,
            defense: 2,
            durability: 165,
        },
        Armor {
            id: items::IRON_CHESTPLATE,
            slot: Chestplate,
            material: Iron,
            defense: 6,
            durability: 240,
        },
        Armor {
            id: items::IRON_LEGGINGS,
            slot: Leggings,
            material: Iron,
            defense: 5,
            durability: 225,
        },
        Armor {
            id: items::IRON_BOOTS,
            slot: Boots,
            material: Iron,
            defense: 2,
            durability: 195,
        },
        Armor {
            id: items::DIAMOND_HELMET,
            slot: Helmet,
            material: Diamond,
            defense: 3,
            durability: 363,
        },
        Armor {
            id: items::DIAMOND_CHESTPLATE,
            slot: Chestplate,
            material: Diamond,
            defense: 8,
            durability: 528,
        },
        Armor {
            id: items::DIAMOND_LEGGINGS,
            slot: Leggings,
            material: Diamond,
            defense: 6,
            durability: 495,
        },
        Armor {
            id: items::DIAMOND_BOOTS,
            slot: Boots,
            material: Diamond,
            defense: 3,
            durability: 429,
        },
        Armor {
            id: items::GOLDEN_HELMET,
            slot: Helmet,
            material: Gold,
            defense: 2,
            durability: 77,
        },
        Armor {
            id: items::GOLDEN_CHESTPLATE,
            slot: Helmet,
            material: Gold,
            defense: 5,
            durability: 112,
        },
        Armor {
            id: items::GOLDEN_LEGGINGS,
            slot: Helmet,
            material: Gold,
            defense: 3,
            durability: 105,
        },
        Armor {
            id: items::GOLDEN_BOOTS,
            slot: Helmet,
            material: Gold,
            defense: 1,
            durability: 91,
        },
    ]
}

pub fn total_defense(
    item_registry: &ItemRegistry,
    helmet: i16,
    chest: i16,
    legs: i16,
    boots: i16,
) -> i32 {
    [helmet, chest, legs, boots]
        .iter()
        .filter_map(|&id| item_registry.armor_defense(id))
        .sum()
}

pub fn apply_armor_reduction(damage: f32, total_armor: i32) -> f32 {
    let armor = total_armor.clamp(0, 20) as f32;
    damage * (1.0 - armor * 0.04)
}
