use crate::blocks::Block;
use std::{fmt::Debug, sync::Arc};

pub trait WorldGenerator: Debug + Send + Sync {
    fn block_at(&self, x: i32, y: u8, z: i32) -> Block;

    fn name(&self) -> &'static str;

    fn spawn_y(&self) -> f64;

    fn level_type(&self) -> &'static str {
        "flat"
    }

    fn is_empty(&self) -> bool {
        false
    }
}

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

    pub fn with_layers(layers: Vec<Block>) -> Self {
        Self { layers }
    }

    pub fn get(&self, y: u8) -> Block {
        self.layers
            .get(y as usize)
            .cloned()
            .unwrap_or_else(Block::air)
    }
}
impl Default for FlatWorldGenerator {
    fn default() -> Self {
        Self::new()
    }
}
impl WorldGenerator for FlatWorldGenerator {
    fn block_at(&self, _x: i32, y: u8, _z: i32) -> Block {
        self.get(y)
    }

    fn name(&self) -> &'static str {
        "flat"
    }

    fn spawn_y(&self) -> f64 {
        self.layers.len() as f64 + 0.5
    }
}

#[derive(Debug, Clone, Default)]
pub struct VoidWorldGenerator;

impl VoidWorldGenerator {
    pub fn new() -> Self {
        Self
    }
}

impl WorldGenerator for VoidWorldGenerator {
    fn block_at(&self, _x: i32, _y: u8, _z: i32) -> Block {
        Block::air()
    }

    fn name(&self) -> &'static str {
        "void"
    }

    fn spawn_y(&self) -> f64 {
        65.0
    }

    fn is_empty(&self) -> bool {
        true
    }
}

pub fn from_name(name: &str) -> Arc<dyn WorldGenerator> {
    match name.trim().to_ascii_lowercase().as_str() {
        "void" => Arc::new(VoidWorldGenerator::new()),
        "flat" => Arc::new(FlatWorldGenerator::new()),
        other => {
            eprintln!("[World] unknown generator `{other}`, using `flat`");
            Arc::new(FlatWorldGenerator::new())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_matches_the_layer_stack() {
        let g = FlatWorldGenerator::new();
        assert_eq!(g.block_at(0, 0, 0).id, 7, "bedrock at the bottom");
        assert_eq!(g.block_at(0, 1, 0).id, 3);
        assert_eq!(g.block_at(0, 2, 0).id, 3);
        assert_eq!(g.block_at(0, 3, 0).id, 2, "grass on top");
        assert!(g.block_at(0, 4, 0).is_air(), "nothing above the stack");
        assert!(g.block_at(0, 255, 0).is_air());
        assert_eq!(g.block_at(-9000, 3, 9000).id, 2);
    }

    #[test]
    fn flat_spawns_on_top_of_its_layers() {
        assert_eq!(FlatWorldGenerator::new().spawn_y(), 4.5);
        assert_eq!(
            FlatWorldGenerator::with_layers(vec![Block::new(7, 0)]).spawn_y(),
            1.5
        );
    }

    #[test]
    fn void_is_air_everywhere() {
        let g = VoidWorldGenerator::new();
        for y in [0u8, 1, 4, 64, 128, 255] {
            assert!(g.block_at(0, y, 0).is_air(), "y={y} was not air");
        }
        assert!(g.block_at(i32::MIN, 0, i32::MAX).is_air());
        assert!(g.is_empty());
        assert!(!FlatWorldGenerator::new().is_empty());
    }

    #[test]
    fn names_resolve_both_ways() {
        assert_eq!(from_name("void").name(), "void");
        assert_eq!(from_name("VOID").name(), "void");
        assert_eq!(from_name("flat").name(), "flat");
        // an unknown generator must not stop the server booting
        assert_eq!(from_name("amplified").name(), "flat");
    }

    #[test]
    fn both_report_a_level_type_the_client_understands() {
        assert_eq!(from_name("flat").level_type(), "flat");
        assert_eq!(from_name("void").level_type(), "flat");
    }
}
