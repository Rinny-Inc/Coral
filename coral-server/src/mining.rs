use coral_types::ToolMaterial;
use coral_world::blocks::registry::BlockRegistry;

use crate::items::ItemRegistry;

pub fn can_harvest(
    item_registry: &ItemRegistry,
    block_registry: &BlockRegistry,
    item_id: i16,
    block_id: u8,
) -> bool {
    let Some(needed) = block_registry.required_material(block_id) else {
        return true;
    };
    item_registry
        .get(item_id)
        .and_then(|item| item.tool_material())
        .is_some_and(|have| material_meets(have, needed))
}

pub fn material_meets(have: ToolMaterial, need: ToolMaterial) -> bool {
    let rank = |m: ToolMaterial| match m {
        ToolMaterial::Wood | ToolMaterial::Gold => 0,
        ToolMaterial::Stone => 1,
        ToolMaterial::Iron => 2,
        ToolMaterial::Diamond => 3,
        ToolMaterial::Any => 0,
    };
    rank(have) >= rank(need)
}

const HARVEST_PENALTY: f32 = 1.5;
const NO_HARVEST_PENALTY: f32 = 5.0;

pub fn break_time_ticks(
    item_registry: &ItemRegistry,
    block_registry: &BlockRegistry,
    item_id: i16,
    block_id: u8,
    in_water: bool,
    on_ground: bool,
) -> u32 {
    let hardness = block_registry.hardness(block_id);
    if hardness <= 0.0 {
        return 0;
    }
    if hardness.is_infinite() {
        return u32::MAX;
    }
    let speed = item_registry.mining_speed(item_id, block_id);

    let penalty = if can_harvest(item_registry, block_registry, item_id, block_id) {
        HARVEST_PENALTY
    } else {
        NO_HARVEST_PENALTY
    };
    let mut seconds = (hardness * penalty) / speed;

    if in_water {
        seconds *= 5.0;
    }
    if !on_ground {
        seconds *= 5.0;
    }
    (seconds * 20.0).ceil() as u32
}

#[cfg(test)]
mod tests {
    use coral_world::blocks::registry::BlockRegistry;

    use super::{break_time_ticks, can_harvest, material_meets};
    use crate::items::ItemRegistry;

    const STONE: u8 = 1;
    const DIRT: u8 = 3;
    const BARE_HAND: i16 = -1;
    const WOODEN_PICKAXE: i16 = 270;
    const DIAMOND_PICKAXE: i16 = 278;

    fn registries() -> (ItemRegistry, BlockRegistry) {
        (ItemRegistry::new(), BlockRegistry::new())
    }

    #[test]
    fn a_better_material_meets_a_lesser_requirement() {
        use coral_types::ToolMaterial::*;
        assert!(material_meets(Diamond, Iron));
        assert!(material_meets(Iron, Stone));
        assert!(material_meets(Stone, Wood));
        assert!(material_meets(Wood, Any));
        assert!(material_meets(Wood, Wood));
    }

    #[test]
    fn a_worse_material_does_not() {
        use coral_types::ToolMaterial::*;
        assert!(!material_meets(Wood, Stone));
        assert!(!material_meets(Stone, Iron));
        assert!(!material_meets(Iron, Diamond));
        assert!(!material_meets(Gold, Stone));
    }

    #[test]
    fn a_block_needing_nothing_can_be_harvested_by_hand() {
        let (items, blocks) = registries();
        assert!(can_harvest(&items, &blocks, BARE_HAND, DIRT));
    }

    #[test]
    fn stone_needs_a_pickaxe() {
        let (items, blocks) = registries();
        assert!(!can_harvest(&items, &blocks, BARE_HAND, STONE));
        assert!(can_harvest(&items, &blocks, WOODEN_PICKAXE, STONE));
    }

    #[test]
    fn breaking_something_you_cannot_harvest_takes_longer() {
        let (items, blocks) = registries();
        let by_hand = break_time_ticks(&items, &blocks, BARE_HAND, STONE, false, true);
        let with_tool = break_time_ticks(&items, &blocks, WOODEN_PICKAXE, STONE, false, true);
        assert!(
            by_hand > with_tool,
            "bare hand {by_hand} ticks vs pickaxe {with_tool}"
        );
    }

    #[test]
    fn the_penalty_is_the_ratio_the_client_uses() {
        let (items, blocks) = registries();
        let by_hand = break_time_ticks(&items, &blocks, BARE_HAND, STONE, false, true) as f32;
        let hardness = blocks.hardness(STONE);
        assert_eq!(by_hand, (hardness * 5.0 * 20.0).ceil());
    }

    #[test]
    fn a_harvesting_tool_is_unchanged_from_before() {
        let (items, blocks) = registries();
        let ticks = break_time_ticks(&items, &blocks, DIAMOND_PICKAXE, STONE, false, true);
        let speed = items.mining_speed(DIAMOND_PICKAXE, STONE);
        let expected = ((blocks.hardness(STONE) * 1.5 / speed) * 20.0).ceil() as u32;
        assert_eq!(ticks, expected);
    }

    #[test]
    fn water_and_being_airborne_each_cost_five_times() {
        let (items, blocks) = registries();
        let speed = items.mining_speed(DIAMOND_PICKAXE, STONE);
        let seconds = blocks.hardness(STONE) * 1.5 / speed;
        let ticks = |multiplier: f32| (seconds * multiplier * 20.0).ceil() as u32;

        let wet = break_time_ticks(&items, &blocks, DIAMOND_PICKAXE, STONE, true, true);
        let airborne = break_time_ticks(&items, &blocks, DIAMOND_PICKAXE, STONE, false, false);
        let both = break_time_ticks(&items, &blocks, DIAMOND_PICKAXE, STONE, true, false);

        assert_eq!(wet, ticks(5.0));
        assert_eq!(airborne, ticks(5.0));
        assert_eq!(both, ticks(25.0));

        let base = break_time_ticks(&items, &blocks, DIAMOND_PICKAXE, STONE, false, true);
        assert!(wet > base && both > wet);
    }

    #[test]
    fn an_instant_block_is_still_instant() {
        let (items, blocks) = registries();
        for block in 0..=255u8 {
            if blocks.hardness(block) <= 0.0 {
                assert_eq!(
                    break_time_ticks(&items, &blocks, BARE_HAND, block, true, false),
                    0,
                    "block {block}"
                );
            }
        }
    }
}
