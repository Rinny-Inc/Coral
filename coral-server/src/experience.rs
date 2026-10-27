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

#[cfg(test)]
mod level_tests {
    use super::{xp_needed_for_level, xp_to_level};

    fn total_at(level: i32) -> i32 {
        (0..level).map(xp_needed_for_level).sum()
    }

    #[test]
    fn the_per_level_cost_matches_the_client() {
        assert_eq!(xp_needed_for_level(0), 7);
        assert_eq!(xp_needed_for_level(14), 35);
        assert_eq!(xp_needed_for_level(15), 37);
        assert_eq!(xp_needed_for_level(29), 107);
        assert_eq!(xp_needed_for_level(30), 112);
        assert_eq!(xp_needed_for_level(40), 202);
    }

    #[test]
    fn no_experience_is_level_zero_with_an_empty_bar() {
        assert_eq!(xp_to_level(0), (0, 0.0));
    }

    #[test]
    fn a_boundary_total_is_the_new_level_with_an_empty_bar() {
        for level in 0..60 {
            let (got_level, progress) = xp_to_level(total_at(level));
            assert_eq!(got_level, level, "total {} ", total_at(level));
            assert_eq!(progress, 0.0, "level {level} did not start empty");
        }
    }

    #[test]
    fn one_short_of_a_boundary_is_the_level_below_with_a_nearly_full_bar() {
        for level in 1..60 {
            let (got_level, progress) = xp_to_level(total_at(level) - 1);
            assert_eq!(got_level, level - 1, "one before level {level}");
            let needed = xp_needed_for_level(level - 1) as f32;
            assert_eq!(progress, (needed - 1.0) / needed, "level {level}");
        }
    }

    #[test]
    fn the_bar_never_leaves_its_range() {
        for total in 0..6000 {
            let (level, progress) = xp_to_level(total);
            assert!(level >= 0, "total {total} gave level {level}");
            assert!(
                (0.0..1.0).contains(&progress),
                "total {total} gave a bar of {progress}"
            );
        }
    }

    #[test]
    fn the_level_never_goes_backwards_as_experience_is_added() {
        let mut last = 0;
        for total in 0..6000 {
            let (level, _) = xp_to_level(total);
            assert!(
                level >= last,
                "total {total} dropped from {last} to {level}"
            );
            last = level;
        }
    }
}
