use coral_types::{ToolKind, ToolMaterial};

use crate::blocks::BlockBehavior;

pub struct Air;
impl BlockBehavior for Air {
    fn id(&self) -> u8 {
        0
    }
    fn hardness(&self) -> f32 {
        0.0
    }
    fn is_solid(&self) -> bool {
        false
    }
    fn is_transparent(&self) -> bool {
        true
    }
}

pub struct Stone;
impl BlockBehavior for Stone {
    fn id(&self) -> u8 {
        1
    }
    fn hardness(&self) -> f32 {
        1.5
    }
    fn required_tool(&self) -> coral_types::ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<coral_types::ToolMaterial> {
        Some(ToolMaterial::Wood)
    }
}

pub struct Grass;
impl BlockBehavior for Grass {
    fn id(&self) -> u8 {
        2
    }
    fn hardness(&self) -> f32 {
        0.6
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Shovel
    }
}

pub struct Dirt;
impl BlockBehavior for Dirt {
    fn id(&self) -> u8 {
        3
    }
    fn hardness(&self) -> f32 {
        0.5
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Shovel
    }
}

pub struct Cobblestone;
impl BlockBehavior for Cobblestone {
    fn id(&self) -> u8 {
        4
    }
    fn hardness(&self) -> f32 {
        2.0
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Wood)
    }
}

pub struct Planks;
impl BlockBehavior for Planks {
    fn id(&self) -> u8 {
        5
    }
    fn hardness(&self) -> f32 {
        2.0
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Axe
    }
    fn is_flammable(&self) -> bool {
        true
    }
}

pub struct Bedrock;
impl BlockBehavior for Bedrock {
    fn id(&self) -> u8 {
        7
    }
    fn hardness(&self) -> f32 {
        f32::INFINITY
    }
    fn blast_resistance(&self) -> f32 {
        18000000.0
    }
}

pub struct Sand;
impl BlockBehavior for Sand {
    fn id(&self) -> u8 {
        12
    }
    fn hardness(&self) -> f32 {
        0.5
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Shovel
    }
}

pub struct Gravel;
impl BlockBehavior for Gravel {
    fn id(&self) -> u8 {
        13
    }
    fn hardness(&self) -> f32 {
        0.6
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Shovel
    }
}

pub struct GoldOre;
impl BlockBehavior for GoldOre {
    fn id(&self) -> u8 {
        14
    }
    fn hardness(&self) -> f32 {
        3.0
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Iron)
    }
}
