use std::collections::HashMap;

use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy)]
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
    blocks: RwLock<HashMap<(i32, u8, i32), Block>>,
}
impl WorldBlocks {
    pub fn new() -> Self {
        Self {
            blocks: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get(&self, x: i32, y: u8, z: i32) -> Block {
        self.blocks
            .read()
            .await
            .get(&(x, y, z))
            .copied()
            .unwrap_or_else(|| match y {
                0 => Block::new(7, 0),
                1 | 2 => Block::new(3, 0),
                3 => Block::new(2, 0),
                _ => Block::air(),
            })
    }

    pub async fn set(&self, x: i32, y: u8, z: i32, block: Block) {
        let mut blocks = self.blocks.write().await;
        if block.is_air() {
            blocks.remove(&(x, y, z));
        } else {
            blocks.insert((x, y, z), block);
        }
    }
}
