use coral_world::blocks::block_hardness_secs;

use crate::items::ItemRegistry;

pub fn break_time_ticks(
    item_registry: &ItemRegistry,
    item_id: i16,
    block_id: u8,
    in_water: bool,
    on_ground: bool,
) -> u32 {
    let hardness = block_hardness_secs(block_id);
    if hardness <= 0.0 {
        return 0;
    }
    if hardness.is_infinite() {
        return u32::MAX;
    }
    let speed = item_registry.mining_speed(item_id, block_id);
    let mut seconds = (hardness * 1.5) / speed;

    if in_water {
        seconds *= 5.0;
    }
    if !on_ground {
        seconds *= 5.0;
    }
    (seconds * 20.0).ceil() as u32
}
