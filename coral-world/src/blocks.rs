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
