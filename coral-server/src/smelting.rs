use coral_types::ids::items;

// TODO: in the future make a MeltingBlock trait and add it to blocks definitions
pub fn fuel_burn_ticks(item_id: i16) -> Option<i16> {
    Some(match item_id {
        items::COAL => 1600,
        items::COAL_BLOCK => 16000,
        items::STICK => 100,
        items::PLANKS | items::SAPLING | items::LOG | items::LOG2 | items::BOOKSHELF => 300,
        items::LAVA_BUCKET => 20000,
        items::BLAZE_ROD => 2400,
        _ => return None,
    })
}

pub fn smelt_result(item_id: i16) -> Option<(i16, i16)> {
    Some(match item_id {
        items::IRON_ORE => (items::IRON_INGOT, 0),
        items::GOLD_ORE => (items::GOLD_INGOT, 0),
        items::SAND => (items::GLASS, 0),
        items::COBBLESTONE => (items::STONE, 0),
        items::NETHERRACK => (items::NETHER_BRICK, 0),
        items::CLAY => (items::HARDENED_CLAY, 0),
        items::PORKCHOP => (items::COOKED_PORKCHOP, 0),
        items::BEEF => (items::COOKED_BEEF, 0),
        items::CHICKEN => (items::COOKED_CHICKEN, 0),
        items::FISH => (items::COOKED_FISH, 0),
        items::POTATO => (items::BAKED_POTATO, 0),
        _ => return None,
    })
}
