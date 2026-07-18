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
    fn is_replaceable(&self) -> bool {
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

pub struct IronOre;
impl BlockBehavior for IronOre {
    fn id(&self) -> u8 {
        15
    }
    fn hardness(&self) -> f32 {
        3.0
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Stone)
    }
}

pub struct CoalOre;
impl BlockBehavior for CoalOre {
    fn id(&self) -> u8 {
        16
    }
    fn hardness(&self) -> f32 {
        3.0
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Wood)
    }
}

pub struct Log;
impl BlockBehavior for Log {
    fn id(&self) -> u8 {
        17
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

pub struct Leaves;
impl BlockBehavior for Leaves {
    fn id(&self) -> u8 {
        18
    }
    fn hardness(&self) -> f32 {
        0.2
    }
    fn is_transparent(&self) -> bool {
        true
    }
    fn is_flammable(&self) -> bool {
        true
    }
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct Glass;
impl BlockBehavior for Glass {
    fn id(&self) -> u8 {
        20
    }
    fn hardness(&self) -> f32 {
        0.3
    }
    fn is_transparent(&self) -> bool {
        true
    }
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct LapisOre;
impl BlockBehavior for LapisOre {
    fn id(&self) -> u8 {
        21
    }
    fn hardness(&self) -> f32 {
        3.0
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Stone)
    }
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct Sandstone;
impl BlockBehavior for Sandstone {
    fn id(&self) -> u8 {
        24
    }
    fn hardness(&self) -> f32 {
        0.8
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Wood)
    }
}

pub struct Wool;
impl BlockBehavior for Wool {
    fn id(&self) -> u8 {
        35
    }
    fn hardness(&self) -> f32 {
        0.8
    }
    fn is_flammable(&self) -> bool {
        true
    }
}

pub struct GoldBlock;
impl BlockBehavior for GoldBlock {
    fn id(&self) -> u8 {
        41
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

pub struct IronBlock;
impl BlockBehavior for IronBlock {
    fn id(&self) -> u8 {
        42
    }
    fn hardness(&self) -> f32 {
        5.0
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Stone)
    }
}

pub struct Bricks;
impl BlockBehavior for Bricks {
    fn id(&self) -> u8 {
        45
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

pub struct DiamondOre;
impl BlockBehavior for DiamondOre {
    fn id(&self) -> u8 {
        56
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
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct DiamondBlock;
impl BlockBehavior for DiamondBlock {
    fn id(&self) -> u8 {
        57
    }
    fn hardness(&self) -> f32 {
        5.0
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Iron)
    }
}

pub struct CraftingTable;
impl BlockBehavior for CraftingTable {
    fn id(&self) -> u8 {
        58
    }
    fn hardness(&self) -> f32 {
        2.5
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Axe
    }
    fn is_flammable(&self) -> bool {
        true
    }
}

pub struct Farmland;
impl BlockBehavior for Farmland {
    fn id(&self) -> u8 {
        60
    }
    fn hardness(&self) -> f32 {
        0.6
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Shovel
    }
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct Furnace;
impl BlockBehavior for Furnace {
    fn id(&self) -> u8 {
        61
    }
    fn hardness(&self) -> f32 {
        3.5
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Wood)
    }
}

pub struct Obsidian;
impl BlockBehavior for Obsidian {
    fn id(&self) -> u8 {
        49
    }
    fn hardness(&self) -> f32 {
        50.0
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Diamond)
    }
    fn blast_resistance(&self) -> f32 {
        6000.0
    }
}

pub struct Chest;
impl BlockBehavior for Chest {
    fn id(&self) -> u8 {
        54
    }
    fn hardness(&self) -> f32 {
        2.5
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Axe
    }
    fn is_flammable(&self) -> bool {
        true
    }
}

pub struct RedstoneOre;
impl BlockBehavior for RedstoneOre {
    fn id(&self) -> u8 {
        73
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
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct Ice;
impl BlockBehavior for Ice {
    fn id(&self) -> u8 {
        79
    }
    fn hardness(&self) -> f32 {
        0.5
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn is_transparent(&self) -> bool {
        true
    }
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct SnowBlock;
impl BlockBehavior for SnowBlock {
    fn id(&self) -> u8 {
        80
    }
    fn hardness(&self) -> f32 {
        0.2
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Shovel
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Wood)
    }
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct Cactus;
impl BlockBehavior for Cactus {
    fn id(&self) -> u8 {
        81
    }
    fn hardness(&self) -> f32 {
        0.4
    }
    fn is_solid(&self) -> bool {
        false
    }
}

pub struct ClayBlock;
impl BlockBehavior for ClayBlock {
    fn id(&self) -> u8 {
        82
    }
    fn hardness(&self) -> f32 {
        0.6
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Shovel
    }
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct SugarCane;
impl BlockBehavior for SugarCane {
    fn id(&self) -> u8 {
        83
    }
    fn hardness(&self) -> f32 {
        0.0
    }
    fn is_solid(&self) -> bool {
        false
    }
}

pub struct Netherrack;
impl BlockBehavior for Netherrack {
    fn id(&self) -> u8 {
        87
    }
    fn hardness(&self) -> f32 {
        0.4
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Wood)
    }
}

pub struct SoulSand;
impl BlockBehavior for SoulSand {
    fn id(&self) -> u8 {
        88
    }
    fn hardness(&self) -> f32 {
        0.5
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Shovel
    }
}

pub struct Glowstone;
impl BlockBehavior for Glowstone {
    fn id(&self) -> u8 {
        89
    }
    fn hardness(&self) -> f32 {
        0.3
    }
    fn is_transparent(&self) -> bool {
        true
    }
    fn light_emission(&self) -> u8 {
        15
    }
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct StoneBrick;
impl BlockBehavior for StoneBrick {
    fn id(&self) -> u8 {
        98
    }
    fn hardness(&self) -> f32 {
        1.5
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Wood)
    }
}

pub struct MelonBlock;
impl BlockBehavior for MelonBlock {
    fn id(&self) -> u8 {
        103
    }
    fn hardness(&self) -> f32 {
        1.0
    }
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct Mycelium;
impl BlockBehavior for Mycelium {
    fn id(&self) -> u8 {
        110
    }
    fn hardness(&self) -> f32 {
        0.6
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Shovel
    }
    fn drops_self(&self) -> bool {
        false
    }
}

pub struct NetherBrick;
impl BlockBehavior for NetherBrick {
    fn id(&self) -> u8 {
        112
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

pub struct EndStone;
impl BlockBehavior for EndStone {
    fn id(&self) -> u8 {
        121
    }
    fn hardness(&self) -> f32 {
        3.0
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Wood)
    }
}

pub struct CoalBlock;
impl BlockBehavior for CoalBlock {
    fn id(&self) -> u8 {
        173
    }
    fn hardness(&self) -> f32 {
        5.0
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Wood)
    }
    fn is_flammable(&self) -> bool {
        true
    }
}

pub struct HardenedClay;
impl BlockBehavior for HardenedClay {
    fn id(&self) -> u8 {
        172
    }
    fn hardness(&self) -> f32 {
        1.25
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Pickaxe
    }
    fn required_material(&self) -> Option<ToolMaterial> {
        Some(ToolMaterial::Wood)
    }
}
