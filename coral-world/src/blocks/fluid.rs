use coral_types::ids::{blocks, items};

use super::BlockBehavior;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FluidKind {
    Water,
    Lava,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fluid {
    pub kind: FluidKind,
    pub flowing: bool,
}

impl Fluid {
    pub const FLOWING_WATER: Fluid = Fluid {
        kind: FluidKind::Water,
        flowing: true,
    };
    pub const STATIONARY_WATER: Fluid = Fluid {
        kind: FluidKind::Water,
        flowing: false,
    };
    pub const FLOWING_LAVA: Fluid = Fluid {
        kind: FluidKind::Lava,
        flowing: true,
    };
    pub const STATIONARY_LAVA: Fluid = Fluid {
        kind: FluidKind::Lava,
        flowing: false,
    };

    pub fn block_id(&self) -> u8 {
        match (self.kind, self.flowing) {
            (FluidKind::Water, true) => blocks::FLOWING_WATER,
            (FluidKind::Water, false) => blocks::WATER,
            (FluidKind::Lava, true) => blocks::FLOWING_LAVA,
            (FluidKind::Lava, false) => blocks::LAVA,
        }
    }

    pub fn from_block_id(id: u8) -> Option<Fluid> {
        match id {
            blocks::FLOWING_WATER => Some(Fluid::FLOWING_WATER),
            blocks::WATER => Some(Fluid::STATIONARY_WATER),
            blocks::FLOWING_LAVA => Some(Fluid::FLOWING_LAVA),
            blocks::LAVA => Some(Fluid::STATIONARY_LAVA),
            _ => None,
        }
    }

    pub fn flowing_variant(&self) -> Fluid {
        Fluid {
            kind: self.kind,
            flowing: true,
        }
    }

    pub fn bucket_item(&self) -> i16 {
        match self.kind {
            FluidKind::Water => 326,
            FluidKind::Lava => 327,
        }
    }

    pub fn from_bucket_item(item_id: i16) -> Option<Fluid> {
        match item_id {
            items::WATER_BUCKET => Some(Fluid::STATIONARY_WATER),
            items::LAVA_BUCKET => Some(Fluid::STATIONARY_LAVA),
            _ => None,
        }
    }

    pub fn is_water(id: u8) -> bool {
        id == blocks::FLOWING_WATER || id == blocks::WATER
    }
    pub fn is_lava(id: u8) -> bool {
        id == blocks::FLOWING_LAVA || id == blocks::LAVA
    }
    pub fn is_fluid(id: u8) -> bool {
        matches!(id, blocks::FLOWING_WATER..=blocks::LAVA)
    }

    pub fn is_source(id: u8, metadata: u8) -> bool {
        matches!(id, blocks::FLOWING_WATER..=blocks::LAVA)
            && (metadata & 0x8) == 0
            && (metadata & 0x7) == 0
    }

    pub fn flow_level(_id: u8, metadata: u8) -> u8 {
        metadata & 0x7
    }

    pub fn is_falling(metadata: u8) -> bool {
        (metadata & 0x8) != 0
    }

    pub fn same_kind(a: u8, b: u8) -> bool {
        match (Fluid::from_block_id(a), Fluid::from_block_id(b)) {
            (Some(fa), Some(fb)) => fa.kind == fb.kind,
            _ => false,
        }
    }
}

impl BlockBehavior for Fluid {
    fn id(&self) -> u8 {
        self.block_id()
    }
    fn hardness(&self) -> f32 {
        100.0
    }
    fn is_solid(&self) -> bool {
        false
    }
    fn is_transparent(&self) -> bool {
        true
    }
    fn light_emission(&self) -> u8 {
        match self.kind {
            FluidKind::Lava => 15,
            FluidKind::Water => 0,
        }
    }
    fn blast_resistance(&self) -> f32 {
        100.0
    }
    fn drops_self(&self) -> bool {
        false
    }
}

// TODO: in the future use BLOCK#is_replaceable()
pub fn is_replaceable(block_id: u8) -> bool {
    matches!(
        block_id,
        blocks::AIR
            | blocks::SAPLING
            | blocks::TALLGRASS
            | blocks::YELLOW_FLOWER
            | blocks::RED_FLOWER
            | blocks::BROWN_MUSHROOM
            | blocks::RED_MUSHROOM
            | blocks::FIRE
    )
}

// TODO: In the future replace with BLOCK#occludes_ray()
pub fn occludes_ray(block_id: u8) -> bool {
    !matches!(
        block_id,
        blocks::AIR
            | blocks::FLOWING_WATER
            | blocks::WATER
            | blocks::FLOWING_LAVA
            | blocks::LAVA
            | blocks::SAPLING
            | blocks::TALLGRASS
            | blocks::DEADBUSH
            | blocks::YELLOW_FLOWER
            | blocks::RED_FLOWER
            | blocks::BROWN_MUSHROOM
            | blocks::RED_MUSHROOM
            | blocks::DOUBLE_PLANT
            | blocks::TORCH
            | blocks::REDSTONE_TORCH
            | blocks::UNLIT_REDSTONE_TORCH
            | blocks::FIRE
            | blocks::REDSTONE_WIRE
            | blocks::WHEAT
            | blocks::CARROTS
            | blocks::POTATOES
            | blocks::PUMPKIN_STEM
            | blocks::MELON_STEM
            | blocks::NETHER_WART
            | blocks::REEDS
            | blocks::VINE
            | blocks::STANDING_SIGN
            | blocks::WALL_SIGN
            | blocks::LEVER
            | blocks::STONE_BUTTON
            | blocks::WOODEN_BUTTON
            | blocks::RAIL
            | blocks::GOLDEN_RAIL
            | blocks::DETECTOR_RAIL
            | blocks::ACTIVATOR_RAIL
            | blocks::TRIPWIRE
            | blocks::TRIPWIRE_HOOK
            | blocks::PORTAL
            | blocks::END_PORTAL
    )
}

#[cfg(test)]
mod occlusion_tests {
    use super::{is_replaceable, occludes_ray};
    use coral_types::ids::blocks;

    #[test]
    fn solid_blocks_stop_a_ray() {
        for id in [
            blocks::STONE,
            blocks::DIRT,
            blocks::COBBLESTONE,
            blocks::GLASS,
            blocks::LEAVES,
            blocks::CHEST,
            blocks::OAK_STAIRS,
            blocks::STONE_SLAB,
            blocks::IRON_BLOCK,
            blocks::BEDROCK,
        ] {
            assert!(occludes_ray(id), "block {id} should stop a ray");
        }
    }

    #[test]
    fn blocks_without_a_collision_box_let_a_ray_through() {
        for id in [
            blocks::AIR,
            blocks::TORCH,
            blocks::REDSTONE_TORCH,
            blocks::TALLGRASS,
            blocks::YELLOW_FLOWER,
            blocks::WHEAT,
            blocks::CARROTS,
            blocks::REEDS,
            blocks::RAIL,
            blocks::LEVER,
            blocks::STANDING_SIGN,
            blocks::REDSTONE_WIRE,
        ] {
            assert!(!occludes_ray(id), "block {id} should not stop a ray");
        }
    }

    #[test]
    fn occlusion_is_not_replaceability() {
        for id in [
            blocks::TORCH,
            blocks::WHEAT,
            blocks::RAIL,
            blocks::VINE,
            blocks::STANDING_SIGN,
        ] {
            assert!(!occludes_ray(id), "{id} should not stop a ray");
            assert!(!is_replaceable(id), "{id} should not be replaceable");
        }
        for id in 0..=255u8 {
            if is_replaceable(id) {
                assert!(!occludes_ray(id), "replaceable {id} must not occlude");
            }
        }
    }

    #[test]
    fn an_unknown_block_occludes_by_default() {
        assert!(occludes_ray(254));
        assert!(occludes_ray(255));
    }

    #[test]
    fn empty_space_is_always_replaceable() {
        assert!(is_replaceable(blocks::AIR));
        assert!(!is_replaceable(blocks::STONE));
        assert!(!is_replaceable(blocks::CHEST));
    }
}
