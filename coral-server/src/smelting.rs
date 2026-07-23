// TODO: in the future make a MeltingBlock trait and add it to blocks definitions
pub fn fuel_burn_ticks(item_id: i16) -> Option<i16> {
    Some(match item_id {
        263 => 1600,            // coal
        173 => 16000,           // coal block
        280 => 100,             // stick
        5 | 6 | 17 | 47 => 300, // planks, sapling, log, bookshelf
        327 => 20000,           // lava bucket
        369 => 2400,            // blaze rod
        _ => return None,
    })
}

pub fn smelt_result(item_id: i16) -> Option<(i16, i16)> {
    Some(match item_id {
        15 => (265, 0),  // iron ore -> ingot
        14 => (266, 0),  // gold ore -> ingot
        12 => (20, 0),   // sand -> glass
        4 => (1, 0),     // cobblestone -> stone
        87 => (405, 0),  // netherrack -> nether brick
        82 => (172, 0),  // clay -> hardened clay
        319 => (320, 0), // raw porkchop -> cooked
        363 => (364, 0), // raw beef -> steak
        365 => (366, 0), // raw chicken -> cooked
        349 => (350, 0), // raw fish -> cooked
        392 => (393, 0), // potato -> baked
        _ => return None,
    })
}
