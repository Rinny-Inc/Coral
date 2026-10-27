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

    fn generates_in_chunk(&self, _cx: i32, _cz: i32) -> bool {
        !self.is_empty()
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
pub struct VoidWorldGenerator {
    platform: Option<(i32, u8, i32)>,
}

impl VoidWorldGenerator {
    pub fn new() -> Self {
        Self { platform: None }
    }

    pub fn with_spawn_platform(spawn_x: i32, spawn_y: f64, spawn_z: i32) -> Self {
        let below = spawn_y.floor() as i32 - 1;
        let platform = (0..=255)
            .contains(&below)
            .then(|| (spawn_x, below as u8, spawn_z));
        Self { platform }
    }

    pub fn platform(&self) -> Option<(i32, u8, i32)> {
        self.platform
    }
}

impl WorldGenerator for VoidWorldGenerator {
    fn block_at(&self, x: i32, y: u8, z: i32) -> Block {
        match self.platform {
            Some(pos) if pos == (x, y, z) => Block::new(1, 0),
            _ => Block::air(),
        }
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

    fn generates_in_chunk(&self, cx: i32, cz: i32) -> bool {
        self.platform
            .is_some_and(|(x, _, z)| (x >> 4) == cx && (z >> 4) == cz)
    }
}

pub fn from_name(name: &str) -> Arc<dyn WorldGenerator> {
    match name.trim().to_ascii_lowercase().as_str() {
        "void" => Arc::new(VoidWorldGenerator::new()),
        "flat" => Arc::new(FlatWorldGenerator::new()),
        other => {
            tracing::error!("[World] unknown generator `{other}`, using `flat`");
            Arc::new(FlatWorldGenerator::new())
        }
    }
}

pub fn from_name_at_spawn(
    name: &str,
    spawn_x: i32,
    spawn_y: f64,
    spawn_z: i32,
) -> Arc<dyn WorldGenerator> {
    match name.trim().to_ascii_lowercase().as_str() {
        "void" => Arc::new(VoidWorldGenerator::with_spawn_platform(
            spawn_x, spawn_y, spawn_z,
        )),
        other => from_name(other),
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
    fn a_bare_void_is_still_air_everywhere() {
        let g = VoidWorldGenerator::new();
        assert_eq!(g.platform(), None);
        for y in [0u8, 64, 65, 255] {
            assert!(g.block_at(0, y, 0).is_air(), "y={y}");
        }
        assert!(!g.generates_in_chunk(0, 0), "nothing to send");
    }

    #[test]
    fn the_platform_sits_one_block_under_the_spawn() {
        let g = VoidWorldGenerator::with_spawn_platform(0, 65.0, 0);
        assert_eq!(g.platform(), Some((0, 64, 0)));
        assert_eq!(g.block_at(0, 64, 0).id, 1);
        assert!(g.block_at(0, 65, 0).is_air(), "the spawn itself is solid");
    }

    #[test]
    fn it_is_exactly_one_block() {
        let g = VoidWorldGenerator::with_spawn_platform(0, 65.0, 0);
        let mut solid = 0;
        for x in -2..=2 {
            for z in -2..=2 {
                for y in 60..=70u8 {
                    if !g.block_at(x, y, z).is_air() {
                        solid += 1;
                    }
                }
            }
        }
        assert_eq!(solid, 1, "the void grew a platform");
    }

    #[test]
    fn the_platform_follows_the_spawn_column() {
        let g = VoidWorldGenerator::with_spawn_platform(-40, 20.0, 137);
        assert_eq!(g.platform(), Some((-40, 19, 137)));
        assert_eq!(g.block_at(-40, 19, 137).id, 1);
        assert!(g.block_at(0, 64, 0).is_air(), "still air at the origin");
    }

    #[test]
    fn a_spawn_on_the_world_floor_gets_no_platform() {
        let g = VoidWorldGenerator::with_spawn_platform(0, 0.0, 0);
        assert_eq!(g.platform(), None);
        assert!(g.block_at(0, 0, 0).is_air());
    }

    #[test]
    fn only_the_spawn_chunk_reports_content() {
        let g = VoidWorldGenerator::with_spawn_platform(-40, 65.0, 137);
        assert!(g.generates_in_chunk(-3, 8), "(-40, 137) is chunk (-3, 8)");
        assert!(!g.generates_in_chunk(0, 0));
        assert!(!g.generates_in_chunk(-3, 9));
    }

    #[test]
    fn a_flat_world_generates_in_every_chunk() {
        let g = FlatWorldGenerator::new();
        assert!(g.generates_in_chunk(0, 0));
        assert!(g.generates_in_chunk(-1000, 1000));
    }

    #[test]
    fn resolving_at_a_spawn_only_changes_the_void() {
        let v = from_name_at_spawn("void", 5, 65.0, -5);
        assert_eq!(v.name(), "void");
        assert_eq!(v.block_at(5, 64, -5).id, 1);

        let f = from_name_at_spawn("flat", 5, 65.0, -5);
        assert_eq!(f.name(), "flat");
        assert_eq!(from_name_at_spawn("amplified", 0, 65.0, 0).name(), "flat");
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
