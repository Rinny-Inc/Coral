use coral_types::{
    ToolKind, ToolMaterial,
    ids::blocks::{self},
};

use crate::blocks::BlockBehavior;

pub struct Air;
impl BlockBehavior for Air {
    fn id(&self) -> u8 {
        blocks::AIR
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
        blocks::STONE
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
        blocks::GRASS
    }
    fn hardness(&self) -> f32 {
        0.55
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Shovel
    }
}

pub struct Dirt;
impl BlockBehavior for Dirt {
    fn id(&self) -> u8 {
        blocks::DIRT
    }
    fn hardness(&self) -> f32 {
        0.45
    }
    fn required_tool(&self) -> ToolKind {
        ToolKind::Shovel
    }
}

pub struct Cobblestone;
impl BlockBehavior for Cobblestone {
    fn id(&self) -> u8 {
        blocks::COBBLESTONE
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
        blocks::PLANKS
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
        blocks::BEDROCK
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
        blocks::SAND
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
        blocks::GRAVEL
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
        blocks::GOLD_ORE
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
        blocks::IRON_ORE
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
        blocks::COAL_ORE
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
        blocks::LOG
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
        blocks::LEAVES
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
        blocks::GLASS
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
        blocks::LAPIS_ORE
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
        blocks::SANDSTONE
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
        blocks::WOOL
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
        blocks::GOLD_BLOCK
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
        blocks::IRON_BLOCK
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
        blocks::BRICK_BLOCK
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
        blocks::DIAMOND_ORE
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
        blocks::DIAMOND_BLOCK
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
        blocks::CRAFTING_TABLE
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
        blocks::FARMLAND
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
        blocks::FURNACE
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
        blocks::OBSIDIAN
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
        blocks::CHEST
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
        blocks::REDSTONE_ORE
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
        blocks::ICE
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
        blocks::SNOW
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
        blocks::CACTUS
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
        blocks::CLAY
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
        blocks::REEDS
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
        blocks::NETHERRACK
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
        blocks::SOUL_SAND
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
        blocks::GLOWSTONE
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
        blocks::STONEBRICK
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
        blocks::MELON_BLOCK
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
        blocks::MYCELIUM
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
        blocks::NETHER_BRICK
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
        blocks::END_STONE
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
        blocks::COAL_BLOCK
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
        blocks::HARDENED_CLAY
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
