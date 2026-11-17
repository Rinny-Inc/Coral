use coral_types::ToolMaterial;

use super::Item;
use coral_types::ids::{blocks, items};

pub struct Sword {
    pub id: i16,
    pub material: ToolMaterial,
    pub damage: f32,
    pub durability: i16,
}

impl Item for Sword {
    fn id(&self) -> i16 {
        self.id
    }
    fn attack_damage(&self) -> f32 {
        self.damage
    }
    fn max_stack_size(&self) -> u8 {
        1
    }
    fn max_durability(&self) -> Option<i16> {
        Some(self.durability)
    }
    fn mining_speed(&self, block_id: u8) -> f32 {
        if block_id == blocks::WEB { 15.0 } else { 1.0 }
    }
}

pub fn all() -> Vec<Sword> {
    vec![
        Sword {
            id: items::WOODEN_SWORD,
            material: ToolMaterial::Wood,
            damage: 4.0,
            durability: 59,
        },
        Sword {
            id: items::STONE_SWORD,
            material: ToolMaterial::Stone,
            damage: 5.0,
            durability: 131,
        },
        Sword {
            id: items::IRON_SWORD,
            material: ToolMaterial::Iron,
            damage: 6.0,
            durability: 250,
        },
        Sword {
            id: items::GOLDEN_SWORD,
            material: ToolMaterial::Gold,
            damage: 4.0,
            durability: 32,
        },
        Sword {
            id: items::DIAMOND_SWORD,
            material: ToolMaterial::Diamond,
            damage: 7.0,
            durability: 1561,
        },
    ]
}
