use coral_types::{ToolKind, ToolMaterial};

use super::Item;

pub struct Tool {
    pub id: i16,
    pub kind: ToolKind,
    pub material: ToolMaterial,
    pub damage: f32,
    pub durability: i16,
}

impl Item for Tool {
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
        let correct = match self.kind {
            ToolKind::Pickaxe => matches!(block_id, 1 | 4 | 16 | 21 | 56 | 73 | 74),
            ToolKind::Axe => matches!(block_id, 17 | 5 | 47 | 162),
            ToolKind::Shovel => matches!(block_id, 2 | 3 | 12 | 13),
            ToolKind::Sword => block_id == 30,
            ToolKind::None => false,
        };
        if !correct {
            return 1.0;
        }
        match self.material {
            ToolMaterial::Wood => 2.0,
            ToolMaterial::Stone => 4.0,
            ToolMaterial::Iron => 6.0,
            ToolMaterial::Diamond => 8.0,
            ToolMaterial::Gold => 12.0,
            ToolMaterial::Any => 1.0,
        }
    }

    fn tool_material(&self) -> Option<ToolMaterial> {
        Some(self.material.clone())
    }
}

pub fn all() -> Vec<Tool> {
    use coral_types::{ToolKind::*, ToolMaterial::*};

    vec![
        // pickaxes
        Tool {
            id: 270,
            kind: Pickaxe,
            material: Wood,
            damage: 2.0,
            durability: 59,
        },
        Tool {
            id: 274,
            kind: Pickaxe,
            material: Stone,
            damage: 3.0,
            durability: 131,
        },
        Tool {
            id: 257,
            kind: Pickaxe,
            material: Iron,
            damage: 4.0,
            durability: 250,
        },
        Tool {
            id: 285,
            kind: Pickaxe,
            material: Gold,
            damage: 2.0,
            durability: 32,
        },
        Tool {
            id: 278,
            kind: Pickaxe,
            material: Diamond,
            damage: 5.0,
            durability: 1561,
        },
        // axes
        Tool {
            id: 271,
            kind: Axe,
            material: Wood,
            damage: 3.0,
            durability: 59,
        },
        Tool {
            id: 275,
            kind: Axe,
            material: Stone,
            damage: 4.0,
            durability: 131,
        },
        Tool {
            id: 258,
            kind: Axe,
            material: Iron,
            damage: 5.0,
            durability: 250,
        },
        Tool {
            id: 286,
            kind: Axe,
            material: Gold,
            damage: 3.0,
            durability: 32,
        },
        Tool {
            id: 279,
            kind: Axe,
            material: Diamond,
            damage: 6.0,
            durability: 1561,
        },
        // shovels
        Tool {
            id: 269,
            kind: Shovel,
            material: Wood,
            damage: 1.0,
            durability: 59,
        },
        Tool {
            id: 273,
            kind: Shovel,
            material: Stone,
            damage: 2.0,
            durability: 131,
        },
        Tool {
            id: 256,
            kind: Shovel,
            material: Iron,
            damage: 3.0,
            durability: 250,
        },
        Tool {
            id: 284,
            kind: Shovel,
            material: Gold,
            damage: 1.0,
            durability: 32,
        },
        Tool {
            id: 277,
            kind: Shovel,
            material: Diamond,
            damage: 4.0,
            durability: 1561,
        },
    ]
}
