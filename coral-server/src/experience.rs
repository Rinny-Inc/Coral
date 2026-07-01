#[derive(Debug, Clone)]
pub struct XpOrb {
    pub entity_id: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vy: f64,
    pub amount: i32,
    pub ticks_alive: u32,
}

pub fn xp_for_block(block_id: u8) -> i32 {
    match block_id {
        16 => random_range(0, 2),
        21 => random_range(2, 5),
        56 => random_range(3, 7),
        73 | 74 => random_range(1, 5),
        129 => random_range(3, 7),
        153 => random_range(2, 5),
        _ => 0,
    }
}

pub fn xp_for_mob_kill(mob_type: &str) -> i32 {
    match mob_type {
        "zombie" | "skeleton" | "spider" | "creeper" | "enderman" => random_range(5, 5),
        "cow" | "pig" | "sheep" | "chicken" => random_range(1, 3),
        _ => 0,
    }
}

fn random_range(min: i32, max: i32) -> i32 {
    if min >= max {
        return min;
    }
    min + (rand::random::<u32>() % (max - min + 1) as u32) as i32
}

pub fn xp_to_level(total_xp: i32) -> (i32, f32) {
    let mut level = 0;
    let mut xp = total_xp;

    loop {
        let needed = xp_needed_for_level(level);
        if xp < needed {
            return (level, xp as f32 / needed as f32);
        }
        xp -= needed;
        level += 1;
    }
}
pub fn xp_needed_for_level(level: i32) -> i32 {
    if level >= 30 {
        return 9 * level - 158;
    }
    if level >= 15 {
        return 5 * level - 38;
    }
    2 * level + 7
}
