use std::collections::BTreeSet;

pub const RAY_GRID: i32 = 16;
pub const RAY_STEP: f64 = 0.3;
pub const RAY_DECAY: f32 = 0.225_000_01;

pub fn explosion_resistance(blast_resistance: f32) -> f32 {
    blast_resistance / 5.0
}

/// The unit directions
pub fn ray_directions() -> impl Iterator<Item = (f64, f64, f64)> {
    (0..RAY_GRID).flat_map(move |i| {
        (0..RAY_GRID).flat_map(move |j| {
            (0..RAY_GRID).filter_map(move |k| {
                let edge = |n: i32| n == 0 || n == RAY_GRID - 1;
                if !(edge(i) || edge(j) || edge(k)) {
                    return None;
                }
                let component = |n: i32| (n as f32 / (RAY_GRID - 1) as f32 * 2.0 - 1.0) as f64;
                let (x, y, z) = (component(i), component(j), component(k));
                let length = (x * x + y * y + z * z).sqrt();
                Some((x / length, y / length, z / length))
            })
        })
    })
}

/// The blocks a blast of `power` at `(x, y, z)` breaks
pub fn destroyed_blocks<F>(
    x: f64,
    y: f64,
    z: f64,
    power: f32,
    rng: &mut dyn FnMut() -> f32,
    resistance_at: F,
) -> BTreeSet<(i32, i32, i32)>
where
    F: Fn(i32, i32, i32) -> Option<f32>,
{
    let mut broken = BTreeSet::new();
    if power <= 0.0 {
        return broken;
    }

    for (dx, dy, dz) in ray_directions() {
        let mut strength = power * (0.7 + rng() * 0.6);
        let (mut rx, mut ry, mut rz) = (x, y, z);

        while strength > 0.0 {
            let block = (rx.floor() as i32, ry.floor() as i32, rz.floor() as i32);
            if let Some(resistance) = resistance_at(block.0, block.1, block.2) {
                strength -= (resistance + 0.3) * 0.3;
            }
            if strength > 0.0 {
                broken.insert(block);
            }
            strength -= RAY_DECAY;
            rx += dx * RAY_STEP;
            ry += dy * RAY_STEP;
            rz += dz * RAY_STEP;
        }
    }
    broken
}

pub fn blast_reach(power: f32) -> i32 {
    if power <= 0.0 {
        return 0;
    }
    let steps = (power * 1.3 / RAY_DECAY) as f64;
    (steps * RAY_STEP).ceil() as i32 + 1
}

pub fn drop_chance(power: f32) -> f32 {
    if power <= 0.0 {
        return 0.0;
    }
    (1.0 / power).min(1.0)
}

pub fn entity_radius(power: f32) -> f64 {
    (power * 2.0) as f64
}

/// How much of an entity the blast can see
pub fn exposure<F>(
    cx: f64,
    cy: f64,
    cz: f64,
    (min_x, min_y, min_z): (f64, f64, f64),
    (max_x, max_y, max_z): (f64, f64, f64),
    solid_at: F,
) -> f64
where
    F: Fn(i32, i32, i32) -> bool,
{
    let step = |lo: f64, hi: f64| 1.0 / ((hi - lo) * 2.0 + 1.0);
    let (sx, sy, sz) = (step(min_x, max_x), step(min_y, max_y), step(min_z, max_z));
    if sx < 0.0 || sy < 0.0 || sz < 0.0 {
        return 0.0;
    }
    let nudge = |s: f64| (1.0 - (1.0 / s).floor() * s) / 2.0;
    let (ox, oz) = (nudge(sx), nudge(sz));

    let (mut clear, mut total) = (0u32, 0u32);
    let mut fx = 0.0;
    while fx <= 1.0 {
        let mut fy = 0.0;
        while fy <= 1.0 {
            let mut fz = 0.0;
            while fz <= 1.0 {
                let from = (
                    min_x + (max_x - min_x) * fx + ox,
                    min_y + (max_y - min_y) * fy,
                    min_z + (max_z - min_z) * fz + oz,
                );
                if !ray_is_blocked(from, (cx, cy, cz), &solid_at) {
                    clear += 1;
                }
                total += 1;
                fz += sz;
            }
            fy += sy;
        }
        fx += sx;
    }
    if total == 0 {
        return 0.0;
    }
    clear as f64 / total as f64
}

/// Whether anything solid sits between two points
fn ray_is_blocked<F>(from: (f64, f64, f64), to: (f64, f64, f64), solid_at: F) -> bool
where
    F: Fn(i32, i32, i32) -> bool,
{
    let (dx, dy, dz) = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
    let distance = (dx * dx + dy * dy + dz * dz).sqrt();
    if distance == 0.0 {
        return false;
    }
    let steps = (distance / 0.5).ceil() as i32;
    for step in 0..=steps {
        let t = step as f64 / steps as f64;
        let (px, py, pz) = (from.0 + dx * t, from.1 + dy * t, from.2 + dz * t);
        if solid_at(px.floor() as i32, py.floor() as i32, pz.floor() as i32) {
            return true;
        }
    }
    false
}

/// of range or exactly on the centre where theres no direction to shove it
pub fn entity_effect(
    distance: f64,
    (dx, dy, dz): (f64, f64, f64),
    power: f32,
    exposure: f64,
) -> Option<(f32, (f64, f64, f64))> {
    let radius = entity_radius(power);
    if radius <= 0.0 {
        return None;
    }
    let ratio = distance / radius;
    if ratio > 1.0 {
        return None;
    }
    let length = (dx * dx + dy * dy + dz * dz).sqrt();
    if length == 0.0 {
        return None;
    }

    let falloff = (1.0 - ratio) * exposure;
    let damage = ((falloff * falloff + falloff) / 2.0 * 8.0 * radius + 1.0) as i32 as f32;
    let shove = (
        dx / length * falloff,
        dy / length * falloff,
        dz / length * falloff,
    );
    Some((damage, shove))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_air(_x: i32, _y: i32, _z: i32) -> Option<f32> {
        None
    }

    fn mid_roll() -> impl FnMut() -> f32 {
        || 0.5
    }

    #[test]
    fn the_ray_fan_is_the_shell_of_the_grid() {
        let rays: Vec<_> = ray_directions().collect();
        assert_eq!(rays.len(), 1352);
        for (x, y, z) in rays {
            let length = (x * x + y * y + z * z).sqrt();
            assert!((length - 1.0).abs() < 1e-9, "ray {x},{y},{z} is not a unit");
        }
    }

    #[test]
    fn the_rays_point_every_which_way() {
        let rays: Vec<_> = ray_directions().collect();
        for axis in 0..3 {
            let component = |r: &(f64, f64, f64)| match axis {
                0 => r.0,
                1 => r.1,
                _ => r.2,
            };
            assert!(
                rays.iter().any(|r| component(r) > 0.9),
                "none point +{axis}"
            );
            assert!(
                rays.iter().any(|r| component(r) < -0.9),
                "none point -{axis}"
            );
        }
    }

    #[test]
    fn a_blast_with_no_power_breaks_nothing() {
        let broken = destroyed_blocks(0.5, 64.5, 0.5, 0.0, &mut mid_roll(), open_air);
        assert!(broken.is_empty());
        assert_eq!(drop_chance(0.0), 0.0);
        assert_eq!(entity_radius(0.0), 0.0);
    }

    #[test]
    fn a_bigger_blast_breaks_more() {
        let sizes: Vec<usize> = [1.0, 2.0, 4.0, 8.0]
            .iter()
            .map(|&power| destroyed_blocks(0.5, 64.5, 0.5, power, &mut mid_roll(), open_air).len())
            .collect();
        for pair in sizes.windows(2) {
            assert!(pair[1] > pair[0], "blast sizes did not grow: {sizes:?}");
        }
    }

    #[test]
    fn a_blast_in_open_air_is_centred_and_roughly_round() {
        let power = 4.0;
        let broken = destroyed_blocks(0.5, 64.5, 0.5, power, &mut mid_roll(), open_air);
        assert!(broken.contains(&(0, 64, 0)), "the centre itself must break");

        let strength = power * (0.7 + 0.5 * 0.6);
        let reach = (strength / RAY_DECAY) as f64 * RAY_STEP + 3.0_f64.sqrt() / 2.0;
        for &(bx, by, bz) in &broken {
            let centre = (bx as f64 + 0.5, by as f64 + 0.5, bz as f64 + 0.5);
            let (dx, dy, dz) = (centre.0 - 0.5, centre.1 - 64.5, centre.2 - 0.5);
            let distance = (dx * dx + dy * dy + dz * dz).sqrt();
            assert!(
                distance <= reach,
                "{bx},{by},{bz} is {distance} away, past {reach}"
            );
        }
    }

    #[test]
    fn bedrock_shrugs_a_blast_off() {
        let bedrock = explosion_resistance(18_000_000.0);
        let broken = destroyed_blocks(0.5, 64.5, 0.5, 4.0, &mut mid_roll(), |_, _, _| {
            Some(bedrock)
        });
        assert!(broken.is_empty(), "bedrock should not break");
    }

    #[test]
    fn obsidian_resists_tnt_but_not_everything() {
        let obsidian = explosion_resistance(6000.0);
        let tnt = destroyed_blocks(0.5, 64.5, 0.5, 4.0, &mut mid_roll(), |_, _, _| {
            Some(obsidian)
        });
        assert!(tnt.is_empty(), "TNT should not break obsidian");

        let huge = destroyed_blocks(0.5, 64.5, 0.5, 4000.0, &mut mid_roll(), |_, _, _| {
            Some(obsidian)
        });
        assert!(
            !huge.is_empty(),
            "a large enough blast should break obsidian"
        );
    }

    #[test]
    fn water_smothers_a_blast() {
        let water = explosion_resistance(500.0);
        assert_eq!(water, 100.0);
        let broken = destroyed_blocks(0.5, 64.5, 0.5, 4.0, &mut mid_roll(), |_, _, _| Some(water));
        assert!(
            broken.is_empty(),
            "a blast under water should break nothing"
        );
    }

    #[test]
    fn stone_is_broken_but_bounds_the_blast() {
        let stone = explosion_resistance(7.5); // hardness 1.5
        let in_stone =
            destroyed_blocks(0.5, 64.5, 0.5, 4.0, &mut mid_roll(), |_, _, _| Some(stone));
        let in_air = destroyed_blocks(0.5, 64.5, 0.5, 4.0, &mut mid_roll(), open_air);
        assert!(!in_stone.is_empty(), "TNT should break stone");
        assert!(
            in_stone.len() < in_air.len(),
            "stone should cost the rays something: {} vs {}",
            in_stone.len(),
            in_air.len()
        );
    }

    #[test]
    fn a_weaker_roll_breaks_no_more_than_a_stronger_one() {
        let stone = explosion_resistance(7.5);
        let weak = destroyed_blocks(0.5, 64.5, 0.5, 4.0, &mut || 0.0, |_, _, _| Some(stone));
        let strong = destroyed_blocks(0.5, 64.5, 0.5, 4.0, &mut || 1.0, |_, _, _| Some(stone));
        assert!(
            weak.len() < strong.len(),
            "the roll should matter: {} vs {}",
            weak.len(),
            strong.len()
        );
    }

    #[test]
    fn the_same_rolls_give_the_same_blast() {
        let stone = explosion_resistance(7.5);
        let a = destroyed_blocks(0.5, 64.5, 0.5, 4.0, &mut mid_roll(), |_, _, _| Some(stone));
        let b = destroyed_blocks(0.5, 64.5, 0.5, 4.0, &mut mid_roll(), |_, _, _| Some(stone));
        assert_eq!(a, b);
    }

    #[test]
    fn the_reach_bounds_every_block_a_blast_breaks() {
        for power in [1.0f32, 2.0, 4.0, 8.0] {
            let reach = blast_reach(power);
            let broken = destroyed_blocks(0.5, 64.5, 0.5, power, &mut || 1.0, open_air);
            for (bx, by, bz) in broken {
                assert!(
                    bx.abs() <= reach && (by - 64).abs() <= reach && bz.abs() <= reach,
                    "power {power} broke {bx},{by},{bz} outside a reach of {reach}"
                );
            }
        }
        assert_eq!(blast_reach(0.0), 0);
    }

    #[test]
    fn drops_thin_out_as_the_blast_grows() {
        assert_eq!(drop_chance(1.0), 1.0);
        assert_eq!(drop_chance(4.0), 0.25);
        assert_eq!(drop_chance(0.5), 1.0);
    }

    #[test]
    fn an_entity_out_of_range_is_untouched() {
        assert!(entity_effect(9.0, (9.0, 0.0, 0.0), 4.0, 1.0).is_none());
        assert!(entity_effect(0.0, (0.0, 0.0, 0.0), 4.0, 1.0).is_none());
    }

    #[test]
    fn damage_falls_off_with_distance() {
        let at = |d: f64| entity_effect(d, (d, 0.0, 0.0), 4.0, 1.0).map(|(dmg, _)| dmg);
        let close = at(0.5).unwrap();
        let mid = at(4.0).unwrap();
        let far = at(7.9).unwrap();
        assert!(close > mid && mid > far, "{close} {mid} {far}");
        assert!(far >= 1.0);
    }

    #[test]
    fn point_blank_tnt_would_kill_an_unarmoured_player() {
        let (damage, _) = entity_effect(0.1, (0.1, 0.0, 0.0), 4.0, 1.0).unwrap();
        assert!(damage > 20.0, "point blank TNT dealt only {damage}");
    }

    #[test]
    fn cover_cuts_damage_and_the_shove() {
        let open = entity_effect(2.0, (2.0, 0.0, 0.0), 4.0, 1.0).unwrap();
        let covered = entity_effect(2.0, (2.0, 0.0, 0.0), 4.0, 0.25).unwrap();
        assert!(covered.0 < open.0, "cover should cut damage");
        assert!(covered.1.0 < open.1.0, "cover should cut the shove");
        let shielded = entity_effect(2.0, (2.0, 0.0, 0.0), 4.0, 0.0).unwrap();
        assert_eq!(shielded.0, 1.0);
        assert_eq!(shielded.1, (0.0, 0.0, 0.0));
    }

    #[test]
    fn the_shove_points_away_from_the_blast() {
        let (_, (vx, vy, vz)) = entity_effect(2.0, (2.0, 0.0, 0.0), 4.0, 1.0).unwrap();
        assert!(vx > 0.0 && vy == 0.0 && vz == 0.0, "{vx} {vy} {vz}");
        let (_, (nx, _, _)) = entity_effect(2.0, (-2.0, 0.0, 0.0), 4.0, 1.0).unwrap();
        assert!(nx < 0.0, "a blast to the east should shove west");
    }

    #[test]
    fn an_entity_in_the_open_is_fully_exposed() {
        let seen = exposure(
            0.5,
            64.5,
            0.5,
            (2.2, 64.0, 0.2),
            (2.8, 65.8, 0.8),
            |_, _, _| false,
        );
        assert_eq!(seen, 1.0);
    }

    #[test]
    fn an_entity_walled_off_is_not_exposed() {
        let seen = exposure(
            0.5,
            64.5,
            0.5,
            (2.2, 64.0, 0.2),
            (2.8, 65.8, 0.8),
            |_, _, _| true,
        );
        assert_eq!(seen, 0.0);
    }

    #[test]
    fn a_wall_between_cuts_exposure_without_erasing_it() {
        let seen = exposure(
            0.5,
            64.5,
            0.5,
            (6.2, 64.0, 0.2),
            (6.8, 65.8, 0.8),
            |x, y, _| x == 3 && y == 64,
        );
        assert!(seen > 0.0 && seen < 1.0, "exposure was {seen}");
    }
}
