use std::collections::HashMap;

use tokio::sync::RwLock;

use crate::generator::FlatWorldGenerator;

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

pub fn block_hardness_secs(block_id: u8) -> f32 {
    match block_id {
        0 => 0.0,
        1 | 4 | 5 | 17 | 47 => 2.0,
        2 | 3 | 13 => 0.6,
        12 => 0.5,
        7 => f32::INFINITY,
        14 | 15 | 16 | 21 | 56 | 73 | 74 => 3.0,
        18 => 0.2,
        20 => 0.3,
        24 | 35 => 0.8,
        _ => 1.0,
    }
}
