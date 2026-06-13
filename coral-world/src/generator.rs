use crate::blocks::Block;

#[derive(Debug, Clone)]
pub struct FlatWorldGenerator {
    layers: Vec<Block>,
}
impl FlatWorldGenerator {
    pub fn new() -> Self {
        Self {
            layers: vec![
                Block::new(7, 0),
                Block::new(3, 0),
                Block::new(3, 0),
                Block::new(2, 0),
            ],
        }
    }

    pub fn spawn_y(&self) -> f64 {
        self.layers.len() as f64 + 0.5
    }

    pub fn get(&self, y: u8) -> Block {
        self.layers
            .get(y as usize)
            .cloned()
            .unwrap_or_else(Block::air)
    }
}
