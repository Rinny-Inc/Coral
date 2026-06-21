use coral_types::ToolMaterial;

#[derive(Debug, Clone, PartialEq)]
pub enum ArmorSlot {
    Helmet,
    Chestplate,
    Leggings,
    Boots,
}

pub struct Armor {
    pub id: i16,
    pub slot: ArmorSlot,
    pub material: ToolMaterial,
    pub defense: i32,
    pub durability: i16,
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

// todo: change wood and stone for leather and chainmail
pub fn all() -> Vec<Armor> {
    use ArmorSlot::*;
    use ToolMaterial::*;

    vec![
        // leather
        Armor {
            id: 298,
            slot: Helmet,
            material: Wood,
            defense: 1,
            durability: 55,
        },
        Armor {
            id: 299,
            slot: Chestplate,
            material: Wood,
            defense: 3,
            durability: 80,
        },
        Armor {
            id: 300,
            slot: Leggings,
            material: Wood,
            defense: 2,
            durability: 75,
        },
        Armor {
            id: 301,
            slot: Boots,
            material: Wood,
            defense: 1,
            durability: 65,
        },
        // chainmail
        Armor {
            id: 302,
            slot: Helmet,
            material: Stone,
            defense: 2,
            durability: 165,
        },
        Armor {
            id: 303,
            slot: Chestplate,
            material: Stone,
            defense: 5,
            durability: 240,
        },
        Armor {
            id: 304,
            slot: Leggings,
            material: Stone,
            defense: 4,
            durability: 225,
        },
        Armor {
            id: 305,
            slot: Boots,
            material: Stone,
            defense: 1,
            durability: 195,
        },
        // iron
        Armor {
            id: 306,
            slot: Helmet,
            material: Iron,
            defense: 2,
            durability: 165,
        },
        Armor {
            id: 307,
            slot: Chestplate,
            material: Iron,
            defense: 6,
            durability: 240,
        },
        Armor {
            id: 308,
            slot: Leggings,
            material: Iron,
            defense: 5,
            durability: 225,
        },
        Armor {
            id: 309,
            slot: Boots,
            material: Iron,
            defense: 2,
            durability: 195,
        },
        // diamond
        Armor {
            id: 310,
            slot: Helmet,
            material: Diamond,
            defense: 3,
            durability: 363,
        },
        Armor {
            id: 311,
            slot: Chestplate,
            material: Diamond,
            defense: 8,
            durability: 528,
        },
        Armor {
            id: 312,
            slot: Leggings,
            material: Diamond,
            defense: 6,
            durability: 495,
        },
        Armor {
            id: 313,
            slot: Boots,
            material: Diamond,
            defense: 3,
            durability: 429,
        },
        // gold
        Armor {
            id: 314,
            slot: Helmet,
            material: Gold,
            defense: 2,
            durability: 77,
        },
        Armor {
            id: 315,
            slot: Helmet,
            material: Gold,
            defense: 5,
            durability: 112,
        },
        Armor {
            id: 316,
            slot: Helmet,
            material: Gold,
            defense: 3,
            durability: 105,
        },
        Armor {
            id: 317,
            slot: Helmet,
            material: Gold,
            defense: 1,
            durability: 91,
        },
    ]
}

pub fn get_armor(item_id: i16) -> Option<Armor> {
    all().into_iter().find(|a| a.id == item_id)
}

pub fn total_defense(helmet: i16, chest: i16, legs: i16, boots: i16) -> i32 {
    [helmet, chest, legs, boots]
        .iter()
        .filter_map(|&id| get_armor(id))
        .map(|a| a.defense)
        .sum()
}

pub fn apply_armor_reduction(damage: f32, total_armor: i32) -> f32 {
    let armor = total_armor.clamp(0, 20) as f32;
    damage * (1.0 - armor * 0.04)
}
