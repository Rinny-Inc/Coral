use crate::generator::FlatWorldGenerator;
use coral_types::{ToolKind, ToolMaterial};
use std::collections::HashMap;
use tokio::sync::RwLock;

pub mod definitions;
pub mod registry;

pub trait BlockBehavior: Send + Sync {
    fn id(&self) -> u8;

    /// Hardness — time in seconds to break by hand.
    /// 0.0 = instant, f32::INFINITY = unbreakable (bedrock).
    fn hardness(&self) -> f32;

    /// Which tool kind is required/preferred.
    fn required_tool(&self) -> ToolKind {
        ToolKind::None
    }

    /// Minimum tool material required to drop anything.
    /// e.g. diamond ore needs iron+ or it drops nothing.
    fn required_material(&self) -> Option<ToolMaterial> {
        None
    }

    /// Whether the block drops itself or delegates to block_drop().
    fn drops_self(&self) -> bool {
        true
    }

    /// Whether this block is solid (used for collision, placement checks).
    fn is_solid(&self) -> bool {
        true
    }

    /// (used for light)
    fn is_transparent(&self) -> bool {
        false
    }

    fn is_flammable(&self) -> bool {
        false
    }

    /// Light level emitted by this block (0-15).
    fn light_emission(&self) -> u8 {
        0
    }

    fn blast_resistance(&self) -> f32 {
        self.hardness() * 5.0
    }
}

#[derive(Debug, Clone)]
pub struct Block {
    pub id: u8,
    pub metadata: u8,
}

impl Block {
    pub fn new(id: u8, metadata: u8) -> Self {
        Self { id, metadata }
    }

    pub fn air() -> Self {
        Self { id: 0, metadata: 0 }
    }

    pub fn is_air(&self) -> bool {
        self.id == 0
    }
}

pub struct WorldBlocks {
    pub blocks: RwLock<HashMap<(i32, u8, i32), Block>>,
}
impl WorldBlocks {
    pub fn new() -> Self {
        Self {
            blocks: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get(&self, x: i32, y: u8, z: i32, generator: &FlatWorldGenerator) -> Block {
        self.blocks
            .read()
            .await
            .get(&(x, y, z))
            .cloned()
            .unwrap_or_else(|| generator.get(y))
    }

    pub async fn set(&self, x: i32, y: u8, z: i32, block: Block) {
        let mut blocks = self.blocks.write().await;
        blocks.insert((x, y, z), block);
    }
}
