use super::{Item, ToolMaterial};

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
        if block_id == 30 { 15.0 } else { 1.0 } // cobweb
    }
}

pub fn all() -> Vec<Sword> {
    vec![
        Sword {
            id: 268,
            material: ToolMaterial::Wood,
            damage: 4.0,
            durability: 59,
        },
        Sword {
            id: 272,
            material: ToolMaterial::Stone,
            damage: 5.0,
            durability: 131,
        },
        Sword {
            id: 267,
            material: ToolMaterial::Iron,
            damage: 6.0,
            durability: 250,
        },
        Sword {
            id: 283,
            material: ToolMaterial::Gold,
            damage: 4.0,
            durability: 32,
        },
        Sword {
            id: 276,
            material: ToolMaterial::Diamond,
            damage: 7.0,
            durability: 1561,
        },
    ]
}
