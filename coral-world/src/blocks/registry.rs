use std::{collections::HashMap, sync::Arc};

use coral_types::{ToolKind, ToolMaterial};

use crate::blocks::BlockBehavior;

pub struct BlockRegistry {
    blocks: HashMap<u8, Arc<dyn BlockBehavior>>,
}

impl BlockRegistry {
    pub fn new() -> Self {
        let mut blocks: HashMap<u8, Arc<dyn BlockBehavior>> = HashMap::new(); // TODO: with capacity once size is known

        macro_rules! register {
            ($($b:expr),* $(,)?) => { // GOD DAMN THIS IS ALIEN
                $(
                    let b = $b;
                    blocks.insert(b.id(), Arc::new(b));
                )*
            };
        }

        use super::definitions::*;
        use super::fluid::Fluid;
        register!(
            Air,
            Stone,
            Grass,
            Dirt,
            Cobblestone,
            Planks,
            Bedrock,
            Sand,
            Gravel,
            GoldOre,
            IronOre,
            CoalOre,
            Log,
            Leaves,
            Glass,
            LapisOre,
            Sandstone,
            Wool,
            GoldBlock,
            IronBlock,
            Bricks,
            DiamondOre,
            DiamondBlock,
            CraftingTable,
            Farmland,
            Furnace,
            Obsidian,
            Chest,
            RedstoneOre,
            Ice,
            SnowBlock,
            Cactus,
            ClayBlock,
            SugarCane,
            Netherrack,
            SoulSand,
            Glowstone,
            StoneBrick,
            MelonBlock,
            Mycelium,
            NetherBrick,
            EndStone,
            CoalBlock,
            HardenedClay,
            Fluid::FLOWING_WATER,
            Fluid::STATIONARY_WATER,
            Fluid::FLOWING_LAVA,
            Fluid::STATIONARY_LAVA,
        );

        Self { blocks }
    }

    pub fn get(&self, id: u8) -> Option<&Arc<dyn BlockBehavior>> {
        self.blocks.get(&id)
    }

    pub fn hardness(&self, id: u8) -> f32 {
        self.get(id).map(|b| b.hardness()).unwrap_or(1.0)
    }

    pub fn required_tool(&self, id: u8) -> ToolKind {
        self.get(id)
            .map(|b| b.required_tool())
            .unwrap_or(ToolKind::None)
    }

    pub fn required_material(&self, id: u8) -> Option<ToolMaterial> {
        self.get(id).and_then(|b| b.required_material())
    }

    pub fn is_solid(&self, id: u8) -> bool {
        self.get(id).map(|b| b.is_solid()).unwrap_or(true)
    }
}
