use std::collections::HashMap;

use coral_types::ItemInfo;
use coral_types::ids::{blocks, items};

pub fn block_drop(block_id: u8, metadata: u8) -> Option<(i16, u8, i16)> {
    let meta = metadata as i16;
    let wood = meta & 3;
    match block_id {
        blocks::AIR => None,
        blocks::STONE => Some((items::COBBLESTONE, 1, 0)), // stone -> cobblestone
        blocks::GRASS => Some((items::DIRT, 1, 0)),        // grass -> dirt
        blocks::DIRT => Some((items::DIRT, 1, 0)),
        blocks::COBBLESTONE => Some((items::COBBLESTONE, 1, 0)),
        blocks::PLANKS => Some((items::PLANKS, 1, meta)), // planks (keep wood type)
        blocks::BEDROCK => None,
        blocks::GOLD_ORE => Some((items::GOLD_ORE, 1, 0)), // gold ore -> itself (needs Silk Touch; otherwise Fortune applies but still drops ore)
        blocks::IRON_ORE => Some((items::IRON_ORE, 1, 0)), // iron ore -> itself
        blocks::COAL_ORE => Some((items::COAL, 1, 0)),     // coal ore -> coal
        blocks::LOG => Some((items::LOG, 1, wood)),        // log (keep wood type)
        blocks::LEAVES => None, // leaves (sapling/apple chance handled separately)
        blocks::GLASS => None,
        blocks::LAPIS_ORE => Some((items::DYE, 1, 4)), // lapis ore -> lapis dye (meta 4)
        blocks::SANDSTONE => Some((items::SANDSTONE, 1, meta)),
        blocks::WOOL => Some((items::WOOL, 1, meta)), // wool (keep color)
        blocks::YELLOW_FLOWER => Some((items::YELLOW_FLOWER, 1, 0)), // dandelion
        blocks::RED_FLOWER => Some((items::RED_FLOWER, 1, 0)), // rose
        blocks::BROWN_MUSHROOM => Some((items::BROWN_MUSHROOM, 1, 0)),
        blocks::RED_MUSHROOM => Some((items::RED_MUSHROOM, 1, 0)),
        blocks::GOLD_BLOCK => Some((items::GOLD_BLOCK, 1, 0)),
        blocks::IRON_BLOCK => Some((items::IRON_BLOCK, 1, 0)),
        blocks::BRICK_BLOCK => Some((items::BRICK_BLOCK, 1, 0)), // bricks
        blocks::TNT => Some((items::TNT, 1, 0)),
        blocks::BOOKSHELF => Some((items::BOOK, 3, 0)), // bookshelf -> 3 books
        blocks::MOSSY_COBBLESTONE => Some((items::MOSSY_COBBLESTONE, 1, 0)),
        blocks::OBSIDIAN => Some((items::OBSIDIAN, 1, 0)),
        blocks::OAK_STAIRS => Some((items::PLANKS, 1, 0)), // oak stairs -> oak planks
        blocks::CHEST => Some((items::CHEST, 1, 0)),
        blocks::DIAMOND_ORE => Some((items::DIAMOND, 1, 0)), // diamond ore -> diamond
        blocks::DIAMOND_BLOCK => Some((items::DIAMOND_BLOCK, 1, 0)),
        blocks::CRAFTING_TABLE => Some((items::CRAFTING_TABLE, 1, 0)),
        blocks::FARMLAND => Some((items::DIRT, 1, 0)), // farmland -> dirt
        blocks::FURNACE | blocks::LIT_FURNACE => Some((items::FURNACE, 1, 0)),
        blocks::STANDING_SIGN => Some((items::SIGN, 1, 0)), // sign post -> sign item
        blocks::LADDER => Some((items::LADDER, 1, 0)),
        blocks::RAIL => Some((items::RAIL, 1, 0)),
        blocks::STONE_STAIRS => Some((items::STONE_STAIRS, 1, 0)), // cobblestone stairs
        blocks::REDSTONE_ORE | blocks::LIT_REDSTONE_ORE => Some((items::REDSTONE, 1, 0)), // redstone ore -> redstone dust
        blocks::SNOW_LAYER => Some((items::SNOWBALL, 1, 0)), // snow layer -> snowball
        blocks::ICE => None,                                 // ice -> nothing
        blocks::SNOW => Some((items::SNOWBALL, 4, 0)),       // snow block -> 4 snowballs
        blocks::CACTUS => Some((items::CACTUS, 1, 0)),
        blocks::CLAY => Some((items::CLAY_BALL, 4, 0)), // clay block -> 4 clay balls
        blocks::REEDS => Some((items::REEDS, 1, 0)),    // sugar cane
        blocks::NETHERRACK => Some((items::NETHERRACK, 1, 0)),
        blocks::SOUL_SAND => Some((items::SOUL_SAND, 1, 0)),
        blocks::GLOWSTONE => Some((items::GLOWSTONE_DUST, 2, 0)), // glowstone -> 2 glowstone dust (random 2-4 handled by caller)
        blocks::LIT_PUMPKIN => Some((items::LIT_PUMPKIN, 1, 0)),  // jack o lantern
        blocks::MONSTER_EGG => Some((items::STONE, 1, 0)),        // monster egg -> stone
        blocks::STONEBRICK => Some((items::STONEBRICK, 1, meta)), // stone brick (keep variant)
        blocks::BROWN_MUSHROOM_BLOCK => Some((items::BROWN_MUSHROOM_BLOCK, 1, 0)),
        blocks::RED_MUSHROOM_BLOCK => Some((items::RED_MUSHROOM_BLOCK, 1, 0)),
        blocks::GLASS_PANE => None,
        blocks::MELON_BLOCK => Some((items::MELON, 3, 0)), // melon block -> 3 melon slices (random 3-7 handled by caller)
        blocks::BRICK_STAIRS => Some((items::BRICK_BLOCK, 1, 0)), // brick stairs -> bricks
        blocks::STONE_BRICK_STAIRS => Some((items::STONEBRICK, 1, 0)), // stone brick stairs -> stone bricks
        blocks::MYCELIUM => Some((items::DIRT, 1, 0)),                 // mycelium -> dirt
        blocks::WATERLILY => Some((items::WATERLILY, 1, 0)),           // lily pad
        blocks::NETHER_BRICK => Some((items::NETHER_BRICK, 1, 0)),
        blocks::NETHER_BRICK_STAIRS => Some((items::NETHER_BRICK, 1, 0)), // nether brick stairs -> nether brick
        blocks::END_STONE => Some((items::END_STONE, 1, 0)),
        blocks::REDSTONE_LAMP | blocks::LIT_REDSTONE_LAMP => Some((items::REDSTONE_LAMP, 1, 0)), // redstone lamp -> off lamp
        blocks::DOUBLE_WOODEN_SLAB => Some((items::PLANKS, 1, meta)), // double wood slab -> planks (keep type)
        blocks::WOODEN_SLAB => Some((items::WOODEN_SLAB, 1, meta)),   // wood slab (keep type)
        blocks::SANDSTONE_STAIRS => Some((items::SANDSTONE, 1, 0)), // sandstone stairs -> sandstone
        blocks::EMERALD_ORE => Some((items::EMERALD, 1, 0)),        // emerald ore -> emerald
        blocks::EMERALD_BLOCK => Some((items::EMERALD_BLOCK, 1, 0)),
        blocks::SPRUCE_STAIRS => Some((items::PLANKS, 1, 1)), // spruce stairs -> spruce planks
        blocks::BIRCH_STAIRS => Some((items::PLANKS, 1, 2)),  // birch stairs -> birch planks
        blocks::JUNGLE_STAIRS => Some((items::PLANKS, 1, 3)), // jungle stairs -> jungle planks
        blocks::REDSTONE_BLOCK => Some((items::REDSTONE_BLOCK, 1, 0)), // redstone block -> itself (not redstone dust!)
        blocks::QUARTZ_ORE => Some((items::QUARTZ, 1, 0)), // nether quartz ore -> quartz
        blocks::QUARTZ_BLOCK => Some((items::QUARTZ_BLOCK, 1, meta)),
        blocks::QUARTZ_STAIRS => Some((items::QUARTZ_BLOCK, 1, 0)), // quartz stairs -> quartz block
        blocks::STAINED_HARDENED_CLAY => Some((items::STAINED_HARDENED_CLAY, 1, meta)), // stained clay (keep color)
        blocks::STAINED_GLASS_PANE => None,
        blocks::LEAVES2 => None, // acacia/dark oak leaves
        blocks::LOG2 => Some((items::LOG2, 1, wood)), // acacia/dark oak log
        blocks::ACACIA_STAIRS => Some((items::PLANKS, 1, 4)), // acacia stairs -> acacia planks
        blocks::DARK_OAK_STAIRS => Some((items::PLANKS, 1, 5)), // dark oak stairs -> dark oak planks
        blocks::HAY_BLOCK => Some((items::HAY_BLOCK, 1, meta)),
        blocks::HARDENED_CLAY => Some((items::HARDENED_CLAY, 1, 0)),
        blocks::COAL_BLOCK => Some((items::COAL_BLOCK, 1, 0)),
        blocks::PACKED_ICE => None,
        blocks::RED_SANDSTONE => Some((items::RED_SANDSTONE, 1, meta)),
        blocks::RED_SANDSTONE_STAIRS => Some((items::RED_SANDSTONE, 1, 0)), // red sandstone stairs -> red sandstone
        _ => Some((block_id as i16, 1, meta)), // default: drop self with metadata
    }
}

pub fn chunk_of(x: f64, z: f64) -> (i32, i32) {
    ((x.floor() as i32) >> 4, (z.floor() as i32) >> 4)
}

#[derive(Debug, Default)]
pub struct DroppedItems {
    by_eid: HashMap<i32, ItemInfo>,
    by_chunk: HashMap<(i32, i32), Vec<i32>>,
}
impl DroppedItems {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, info: ItemInfo) {
        let (eid, x, _, z, ..) = info;
        if self.by_eid.insert(eid, info).is_none() {
            self.by_chunk.entry(chunk_of(x, z)).or_default().push(eid);
        }
    }

    pub fn remove(&mut self, eid: i32) -> Option<ItemInfo> {
        let info = self.by_eid.remove(&eid)?;
        let chunk = chunk_of(info.1, info.3);
        if let Some(here) = self.by_chunk.get_mut(&chunk) {
            here.retain(|other| *other != eid);
            if here.is_empty() {
                self.by_chunk.remove(&chunk);
            }
        }
        Some(info)
    }

    pub fn set_count(&mut self, eid: i32, count: u8) {
        if let Some(info) = self.by_eid.get_mut(&eid) {
            info.5 = count;
        }
    }

    pub fn candidates_near(&self, x: f64, z: f64, radius: f64) -> impl Iterator<Item = &ItemInfo> {
        let radius = radius.max(0.0);
        let (min_cx, min_cz) = chunk_of(x - radius, z - radius);
        let (max_cx, max_cz) = chunk_of(x + radius, z + radius);

        (min_cx..=max_cx)
            .flat_map(move |cx| (min_cz..=max_cz).map(move |cz| (cx, cz)))
            .filter_map(move |key| self.by_chunk.get(&key))
            .flatten()
            .filter_map(move |eid| self.by_eid.get(eid))
    }

    pub fn len(&self) -> usize {
        self.by_eid.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_eid.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(eid: i32, x: f64, z: f64) -> ItemInfo {
        (eid, x, 64.0, z, 1, 1, 0)
    }

    fn eids_near(items: &DroppedItems, x: f64, z: f64, radius: f64) -> Vec<i32> {
        let mut found: Vec<i32> = items.candidates_near(x, z, radius).map(|i| i.0).collect();
        found.sort_unstable();
        found
    }

    #[test]
    fn a_sideways_log_drops_as_its_wood_type() {
        for wood in 0..4u8 {
            for axis in 0..3u8 {
                let placed = wood | (axis << 2);
                assert_eq!(
                    block_drop(17, placed),
                    Some((17, 1, wood as i16)),
                    "log metadata {placed} must drop as wood type {wood}"
                );
            }
        }

        for wood in 0..2u8 {
            for axis in 0..3u8 {
                let placed = wood | (axis << 2);
                assert_eq!(
                    block_drop(162, placed),
                    Some((162, 1, wood as i16)),
                    "log2 metadata {placed} must drop as wood type {wood}"
                );
            }
        }
    }

    #[test]
    fn an_empty_index_has_nothing_near_anything() {
        let items = DroppedItems::new();
        assert!(items.is_empty());
        assert_eq!(eids_near(&items, 0.0, 0.0, 16.0), Vec::<i32>::new());
    }

    #[test]
    fn an_item_is_found_in_its_own_chunk() {
        let mut items = DroppedItems::new();
        items.insert(item(1, 8.0, 8.0));
        assert_eq!(eids_near(&items, 8.0, 8.0, 1.0), vec![1]);
    }

    #[test]
    fn an_item_in_a_distant_chunk_is_not() {
        let mut items = DroppedItems::new();
        items.insert(item(1, 1000.0, 1000.0));
        assert_eq!(eids_near(&items, 0.0, 0.0, 1.0), Vec::<i32>::new());
    }

    #[test]
    fn negative_coordinates_land_in_the_chunk_below_zero() {
        let mut items = DroppedItems::new();
        items.insert(item(1, -1.0, -1.0));
        assert_eq!(eids_near(&items, -1.0, -1.0, 1.0), vec![1]);
        assert_eq!(eids_near(&items, 20.0, 20.0, 1.0), Vec::<i32>::new());
    }

    #[test]
    fn removing_an_item_takes_it_out_of_its_chunk_too() {
        let mut items = DroppedItems::new();
        items.insert(item(1, 8.0, 8.0));
        items.insert(item(2, 8.0, 8.0));

        assert_eq!(items.remove(1).map(|i| i.0), Some(1));

        assert_eq!(eids_near(&items, 8.0, 8.0, 1.0), vec![2]);
        assert_eq!(items.len(), 1);
        assert!(items.remove(1).is_none(), "removed twice");
    }

    #[test]
    fn emptying_a_chunk_drops_its_bucket() {
        let mut items = DroppedItems::new();
        items.insert(item(1, 8.0, 8.0));
        items.remove(1);
        assert!(items.by_chunk.is_empty(), "the bucket outlived its item");
    }

    #[test]
    fn inserting_the_same_id_twice_does_not_duplicate_it() {
        let mut items = DroppedItems::new();
        items.insert(item(1, 8.0, 8.0));
        items.insert(item(1, 8.0, 8.0));
        assert_eq!(eids_near(&items, 8.0, 8.0, 1.0), vec![1]);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn a_partial_pickup_leaves_the_item_where_it_was() {
        let mut items = DroppedItems::new();
        items.insert((1, 8.0, 64.0, 8.0, 264, 10, 0));

        items.set_count(1, 3);

        let found: Vec<&ItemInfo> = items.candidates_near(8.0, 8.0, 1.0).collect();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].5, 3, "count did not change");
        assert_eq!((found[0].1, found[0].3), (8.0, 8.0), "it moved");
    }

    #[test]
    fn a_radius_spanning_chunks_reaches_all_of_them() {
        let mut items = DroppedItems::new();
        for (i, (cx, cz)) in (-1..=1)
            .flat_map(|x| (-1..=1).map(move |z| (x, z)))
            .enumerate()
        {
            items.insert(item(i as i32, (cx * 16 + 8) as f64, (cz * 16 + 8) as f64));
        }

        assert_eq!(eids_near(&items, 8.0, 8.0, 1.0).len(), 1, "own chunk only");
        assert_eq!(eids_near(&items, 8.0, 8.0, 24.0).len(), 9, "all nine");
    }

    #[test]
    fn it_never_misses_an_item_a_full_scan_would_find() {
        // xorshift64*, so a failure is reproducible
        let mut state = 0x5EEDu64;
        let mut next = || {
            state ^= state >> 12;
            state ^= state << 25;
            state ^= state >> 27;
            (state.wrapping_mul(0x2545_F491_4F6C_DD1D) % 4000) as f64 - 2000.0
        };

        let mut items = DroppedItems::new();
        let mut all = Vec::new();
        for eid in 0..400 {
            let info = item(eid, next(), next());
            all.push(info);
            items.insert(info);
        }
        // and take a few back out again
        for eid in (0..400).step_by(7) {
            items.remove(eid);
            all.retain(|i| i.0 != eid);
        }

        for _ in 0..300 {
            let (qx, qz) = (next(), next());
            for radius in [0.5, 1.0, 4.0, 16.0] {
                let found = eids_near(&items, qx, qz, radius);
                for info in &all {
                    let (dx, dz) = (info.1 - qx, info.3 - qz);
                    if (dx * dx + dz * dz).sqrt() <= radius {
                        assert!(
                            found.contains(&info.0),
                            "missed {} at ({}, {}) from ({qx}, {qz}) r={radius}",
                            info.0,
                            info.1,
                            info.3
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod pickup_reach_tests {
    use super::*;
    use crate::bounding_box::EntityBounds;

    #[test]
    fn two_blocks_covers_any_possible_overlap() {
        let player = EntityBounds::player(false);
        let item = EntityBounds::item();
        // the widest horizontal gap at which the boxes still touch
        let max_overlap = player.width / 2.0 + item.width / 2.0;
        assert!(
            max_overlap <= 2.0,
            "a reach of 2.0 is too short for boxes {} and {} wide",
            player.width,
            item.width
        );
    }

    #[test]
    fn the_query_reaches_the_neighbouring_chunk_from_a_border() {
        let mut items = DroppedItems::new();
        items.insert((1, -0.1, 64.0, 8.0, 1, 1, 0));

        let found: Vec<i32> = items.candidates_near(0.1, 8.0, 2.0).map(|i| i.0).collect();
        assert_eq!(found, vec![1], "missed an item just over the chunk line");
    }
}
