use std::{
    collections::HashMap,
    path::Path,
    sync::{Arc, atomic::AtomicI64},
};

use crate::{
    blocks::{Block, WorldBlocks},
    generator::{FlatWorldGenerator, WorldGenerator},
    nbt::NbtTag,
};

pub struct World {
    pub blocks: WorldBlocks,
    pub generator: Arc<dyn WorldGenerator>,
    pub time: Arc<AtomicI64>,
}
impl World {
    pub fn new(generator: Arc<dyn WorldGenerator>) -> Self {
        Self {
            blocks: WorldBlocks::new(),
            generator,
            time: Arc::new(AtomicI64::new(0)),
        }
    }

    pub fn flat() -> Self {
        Self::new(Arc::new(FlatWorldGenerator::new()))
    }

    pub fn generator_name(&self) -> &'static str {
        self.generator.name()
    }

    pub fn level_type(&self) -> &'static str {
        self.generator.level_type()
    }

    pub fn spawn_y(&self) -> f64 {
        self.generator.spawn_y()
    }

    pub async fn get(&self, x: i32, y: u8, z: i32) -> Block {
        self.blocks.get(x, y, z, self.generator.as_ref()).await
    }

    pub async fn set(&self, x: i32, y: u8, z: i32, block: Block) {
        self.blocks.set(x, y, z, block).await;
    }

    pub async fn load(&self, world_dir: &Path) {
        self.blocks.load(world_dir).await;
    }

    pub async fn save(
        &self,
        world_dir: &Path,
        tile_entities_by_chunk: &HashMap<(i32, i32), Vec<NbtTag>>,
    ) {
        self.blocks
            .save(world_dir, self.generator.clone(), tile_entities_by_chunk)
            .await;
    }

    pub async fn surface_y(&self, x: i32, z: i32) -> Option<u8> {
        if self.generator.is_empty() && self.blocks.is_untouched_column(x, z).await {
            return None;
        }
        for y in (0..=255u8).rev() {
            if !self.get(x, y, z).await.is_air() {
                return y.checked_add(1);
            }
        }
        None
    }

    pub async fn is_floating_spawn(&self, x: f64, y: f64, z: f64) -> bool {
        let (bx, bz) = (x.floor() as i32, z.floor() as i32);
        let feet = y.floor().clamp(0.0, 255.0) as u8;
        // standing on something, or inside something, is not floating
        if feet > 0 && !self.get(bx, feet - 1, bz).await.is_air() {
            return false;
        }
        if !self.get(bx, feet, bz).await.is_air() {
            return false;
        }
        match self.surface_y(bx, bz).await {
            Some(surface) => (feet as i32) - (surface as i32) > 1,
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generator::VoidWorldGenerator;

    fn void() -> World {
        World::new(Arc::new(VoidWorldGenerator::new()))
    }

    #[tokio::test]
    async fn a_void_world_is_air_all_the_way_down() {
        let w = void();
        for y in [0u8, 1, 4, 64, 255] {
            assert!(w.get(0, y, 0).await.is_air(), "y={y} was not air");
        }
        assert_eq!(w.surface_y(0, 0).await, None);
        assert_eq!(w.spawn_y(), 65.0);
    }

    #[tokio::test]
    async fn a_flat_world_has_its_floor() {
        let w = World::flat();
        assert_eq!(w.get(0, 0, 0).await.id, 7);
        assert_eq!(w.get(0, 3, 0).await.id, 2);
        assert!(w.get(0, 4, 0).await.is_air());
        assert_eq!(w.surface_y(0, 0).await, Some(4));
        assert_eq!(w.spawn_y(), 4.5);
    }

    #[tokio::test]
    async fn edits_win_over_the_generator_in_both_worlds() {
        for w in [void(), World::flat()] {
            w.set(5, 100, 5, Block::new(1, 0)).await;
            assert_eq!(w.get(5, 100, 5).await.id, 1);
            assert_eq!(w.surface_y(5, 5).await, Some(101));
        }
    }

    #[tokio::test]
    async fn a_platform_in_the_void_is_local_to_its_column() {
        let w = void();
        w.set(0, 64, 0, Block::new(1, 0)).await;
        assert_eq!(w.surface_y(0, 0).await, Some(65));
        assert_eq!(w.surface_y(1, 0).await, None);
        assert!(w.get(0, 63, 0).await.is_air());
        assert!(w.get(0, 65, 0).await.is_air());
    }

    #[tokio::test]
    async fn nothing_floats_in_a_void_world() {
        assert!(!void().is_floating_spawn(0.5, 65.0, 0.5).await);
        assert!(!void().is_floating_spawn(0.5, 200.0, 0.5).await);
    }

    #[tokio::test]
    async fn a_spawn_high_above_flat_terrain_still_floats() {
        let w = World::flat();
        assert!(w.is_floating_spawn(0.5, 64.0, 0.5).await);
        assert!(!w.is_floating_spawn(0.5, 4.0, 0.5).await);
        assert!(!w.is_floating_spawn(0.5, 5.0, 0.5).await);
    }

    #[tokio::test]
    async fn a_platform_under_the_spawn_is_not_floating() {
        let w = void();
        w.set(0, 64, 0, Block::new(1, 0)).await;
        assert!(!w.is_floating_spawn(0.5, 65.0, 0.5).await);
    }

    #[tokio::test]
    async fn both_generators_report_a_level_type_the_client_understands() {
        assert_eq!(World::flat().level_type(), "flat");
        assert_eq!(void().level_type(), "flat");
    }
}
