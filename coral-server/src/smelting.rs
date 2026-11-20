use coral_types::ids::items;

// TODO: in the future make a MeltingBlock trait and add it to blocks definitions <- do I really want that???
pub fn fuel_burn_ticks(item_id: i16) -> Option<i16> {
    Some(match item_id {
        items::COAL => 1600,
        items::COAL_BLOCK => 16000,
        items::PLANKS
        | items::LOG
        | items::LOG2
        | items::BOOKSHELF
        | items::CRAFTING_TABLE
        | items::CHEST
        | items::TRAPPED_CHEST
        | items::NOTEBLOCK
        | items::JUKEBOX
        | items::DAYLIGHT_DETECTOR
        | items::TRAPDOOR
        | items::WOODEN_PRESSURE_PLATE
        | items::WOODEN_BUTTON
        | items::FENCE
        | items::SPRUCE_FENCE
        | items::BIRCH_FENCE
        | items::JUNGLE_FENCE
        | items::DARK_OAK_FENCE
        | items::ACACIA_FENCE
        | items::FENCE_GATE
        | items::SPRUCE_FENCE_GATE
        | items::BIRCH_FENCE_GATE
        | items::JUNGLE_FENCE_GATE
        | items::DARK_OAK_FENCE_GATE
        | items::ACACIA_FENCE_GATE
        | items::OAK_STAIRS
        | items::SPRUCE_STAIRS
        | items::BIRCH_STAIRS
        | items::JUNGLE_STAIRS
        | items::ACACIA_STAIRS
        | items::DARK_OAK_STAIRS => 300,
        items::LAVA_BUCKET => 20000,
        items::BLAZE_ROD => 2400,
        items::WOODEN_SLAB => 150,
        items::WOODEN_SWORD
        | items::WOODEN_SHOVEL
        | items::WOODEN_PICKAXE
        | items::WOODEN_AXE
        | items::WOODEN_HOE => 200,
        items::STICK => 100,
        items::SAPLING => 100,
        _ => return None,
    })
}

pub fn fuel_residue(item_id: i16) -> Option<i16> {
    (item_id == items::LAVA_BUCKET).then_some(items::BUCKET)
}

pub fn smelt_result(item_id: i16, metadata: i16) -> Option<(i16, i16)> {
    match (item_id, metadata) {
        (items::SPONGE, 1) => return Some((items::SPONGE, 0)),
        (items::SPONGE, _) => return None,
        (items::FISH, m @ (0 | 1)) => return Some((items::COOKED_FISH, m)),
        (items::FISH, _) => return None,
        _ => {}
    }
    Some(match item_id {
        items::IRON_ORE => (items::IRON_INGOT, 0),
        items::GOLD_ORE => (items::GOLD_INGOT, 0),
        items::DIAMOND_ORE => (items::DIAMOND, 0),
        items::EMERALD_ORE => (items::EMERALD, 0),
        items::COAL_ORE => (items::COAL, 0),
        items::REDSTONE_ORE => (items::REDSTONE, 0),
        items::QUARTZ_ORE => (items::QUARTZ, 0),
        items::LAPIS_ORE => (items::DYE, 4),
        items::CACTUS => (items::DYE, 2),
        items::SAND => (items::GLASS, 0),
        items::COBBLESTONE => (items::STONE, 0),
        items::NETHERRACK => (items::NETHERBRICK, 0),
        items::CLAY => (items::HARDENED_CLAY, 0),
        items::CLAY_BALL => (items::BRICK, 0),
        items::LOG | items::LOG2 => (items::COAL, 1),
        items::PORKCHOP => (items::COOKED_PORKCHOP, 0),
        items::BEEF => (items::COOKED_BEEF, 0),
        items::CHICKEN => (items::COOKED_CHICKEN, 0),
        items::RABBIT => (items::COOKED_RABBIT, 0),
        items::MUTTON => (items::COOKED_MUTTON, 0),
        items::POTATO => (items::BAKED_POTATO, 0),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_of_any_species_becomes_charcoal() {
        for meta in 0..4 {
            assert_eq!(smelt_result(items::LOG, meta), Some((items::COAL, 1)));
            assert_eq!(smelt_result(items::LOG2, meta), Some((items::COAL, 1)));
        }
    }

    #[test]
    fn the_ores_that_only_drop_their_item_still_smelt() {
        assert_eq!(
            smelt_result(items::DIAMOND_ORE, 0),
            Some((items::DIAMOND, 0))
        );
        assert_eq!(
            smelt_result(items::EMERALD_ORE, 0),
            Some((items::EMERALD, 0))
        );
        assert_eq!(smelt_result(items::COAL_ORE, 0), Some((items::COAL, 0)));
        assert_eq!(
            smelt_result(items::REDSTONE_ORE, 0),
            Some((items::REDSTONE, 0))
        );
        assert_eq!(smelt_result(items::QUARTZ_ORE, 0), Some((items::QUARTZ, 0)));
        assert_eq!(smelt_result(items::LAPIS_ORE, 0), Some((items::DYE, 4)));
    }

    #[test]
    fn only_the_wet_sponge_smelts() {
        assert_eq!(smelt_result(items::SPONGE, 1), Some((items::SPONGE, 0)));
        assert_eq!(smelt_result(items::SPONGE, 0), None);
    }

    #[test]
    fn fish_keeps_its_species_and_only_two_can_cook() {
        assert_eq!(smelt_result(items::FISH, 0), Some((items::COOKED_FISH, 0)));
        assert_eq!(smelt_result(items::FISH, 1), Some((items::COOKED_FISH, 1)));
        assert_eq!(smelt_result(items::FISH, 2), None);
        assert_eq!(smelt_result(items::FISH, 3), None);
    }

    #[test]
    fn clay_ball_and_clay_block_are_different_recipes() {
        assert_eq!(smelt_result(items::CLAY_BALL, 0), Some((items::BRICK, 0)));
        assert_eq!(
            smelt_result(items::CLAY, 0),
            Some((items::HARDENED_CLAY, 0))
        );
    }

    #[test]
    fn nothing_smelts_into_itself() {
        for id in items::ALL.iter().map(|(id, _)| *id) {
            for meta in 0..4 {
                if let Some((rid, rmeta)) = smelt_result(id, meta) {
                    assert!((rid, rmeta) != (id, meta), "{id}:{meta} smelts into itself");
                }
            }
        }
    }

    #[test]
    fn fuel_times_match_vanilla() {
        assert_eq!(fuel_burn_ticks(items::LAVA_BUCKET), Some(20000));
        assert_eq!(fuel_burn_ticks(items::COAL_BLOCK), Some(16000));
        assert_eq!(fuel_burn_ticks(items::BLAZE_ROD), Some(2400));
        assert_eq!(fuel_burn_ticks(items::COAL), Some(1600));
        assert_eq!(fuel_burn_ticks(items::PLANKS), Some(300));
        assert_eq!(fuel_burn_ticks(items::LOG), Some(300));
        assert_eq!(fuel_burn_ticks(items::WOODEN_SLAB), Some(150));
        assert_eq!(fuel_burn_ticks(items::WOODEN_PICKAXE), Some(200));
        assert_eq!(fuel_burn_ticks(items::STICK), Some(100));
        assert_eq!(fuel_burn_ticks(items::SAPLING), Some(100));
        assert_eq!(fuel_burn_ticks(items::STONE), None);
        assert_eq!(fuel_burn_ticks(items::IRON_PICKAXE), None);
    }

    #[test]
    fn every_burn_time_is_a_positive_number_of_ticks() {
        for id in items::ALL.iter().map(|(id, _)| *id) {
            if let Some(ticks) = fuel_burn_ticks(id) {
                assert!(ticks > 0, "{id} burns for {ticks}");
            }
        }
    }

    #[test]
    fn only_a_lava_bucket_leaves_something_behind() {
        assert_eq!(fuel_residue(items::LAVA_BUCKET), Some(items::BUCKET));
        assert_eq!(fuel_residue(items::COAL), None);
    }
}
