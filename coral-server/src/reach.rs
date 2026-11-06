use coral_types::GameMode;

use crate::bounding_box::EntityBounds;

pub const SURVIVAL_BLOCK_REACH: f64 = 4.5;
pub const CREATIVE_BLOCK_REACH: f64 = 5.0;

pub const LATENCY_TOLERANCE: f64 = 1.0;

pub fn block_reach_limit(gamemode: GameMode) -> f64 {
    let base = match gamemode {
        GameMode::Creative | GameMode::Spectator => CREATIVE_BLOCK_REACH,
        _ => SURVIVAL_BLOCK_REACH,
    };
    base + LATENCY_TOLERANCE
}

pub fn dist_sq_to_block(eye: (f64, f64, f64), bx: i32, by: i32, bz: i32) -> f64 {
    let nearest = |v: f64, b: i32| v.clamp(b as f64, b as f64 + 1.0);

    let dx = eye.0 - nearest(eye.0, bx);
    let dy = eye.1 - nearest(eye.1, by);
    let dz = eye.2 - nearest(eye.2, bz);

    dx * dx + dy * dy + dz * dz
}

pub fn can_reach_block(
    feet: (f64, f64, f64),
    sneaking: bool,
    gamemode: GameMode,
    bx: i32,
    by: i32,
    bz: i32,
) -> bool {
    if !feet.0.is_finite() || !feet.1.is_finite() || !feet.2.is_finite() {
        return false;
    }

    let eye = (
        feet.0,
        feet.1 + EntityBounds::player(sneaking).height,
        feet.2,
    );
    let limit = block_reach_limit(gamemode);

    dist_sq_to_block(eye, bx, by, bz) <= limit * limit
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standing_at(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        (x, y, z)
    }

    #[test]
    fn the_block_under_your_feet_is_always_in_range() {
        for gamemode in [
            GameMode::Survival,
            GameMode::Creative,
            GameMode::Adventure,
            GameMode::Spectator,
        ] {
            for sneaking in [false, true] {
                assert!(
                    can_reach_block(standing_at(0.5, 64.0, 0.5), sneaking, gamemode, 0, 63, 0),
                    "{gamemode:?} sneaking={sneaking}"
                );
            }
        }
    }

    #[test]
    fn a_block_across_the_room_is_not() {
        assert!(!can_reach_block(
            standing_at(0.5, 64.0, 0.5),
            false,
            GameMode::Survival,
            20,
            64,
            0
        ));
    }

    #[test]
    fn range_is_measured_to_the_near_face_not_the_centre() {
        let feet = standing_at(0.5, 64.0, 0.5);
        let eye = (0.5, 64.0 + EntityBounds::player(false).height, 0.5);
        let limit = block_reach_limit(GameMode::Survival);

        let (bx, by, bz) = (6, 65, 0);
        let to_nearest = dist_sq_to_block(eye, bx, by, bz).sqrt();
        let to_centre = ((bx as f64 + 0.5 - eye.0).powi(2)
            + (by as f64 + 0.5 - eye.1).powi(2)
            + (bz as f64 + 0.5 - eye.2).powi(2))
        .sqrt();

        assert!(
            to_nearest <= limit,
            "near face {to_nearest} is within {limit}"
        );
        assert!(to_centre > limit, "centre {to_centre} is beyond {limit}");
        assert!(
            can_reach_block(feet, false, GameMode::Survival, bx, by, bz),
            "measuring to the centre would have refused this"
        );
    }

    #[test]
    fn a_block_the_eye_is_inside_measures_zero() {
        assert_eq!(dist_sq_to_block((5.5, 64.5, 5.5), 5, 64, 5), 0.0);
    }

    #[test]
    fn negative_coordinates_measure_the_same_as_positive_ones() {
        let positive = dist_sq_to_block((0.5, 64.0, 0.5), 4, 64, 0);
        let negative = dist_sq_to_block((-0.5, 64.0, 0.5), -5, 64, 0);
        assert!(
            (positive - negative).abs() < 1e-9,
            "{positive} vs {negative}"
        );
    }

    #[test]
    fn creative_reaches_further_than_survival() {
        assert!(block_reach_limit(GameMode::Creative) > block_reach_limit(GameMode::Survival));

        let feet = standing_at(1.2, 64.0, 0.5);
        let gap = dist_sq_to_block(
            (1.2, 64.0 + EntityBounds::player(false).height, 0.5),
            7,
            65,
            0,
        )
        .sqrt();
        assert!(
            gap > block_reach_limit(GameMode::Survival)
                && gap <= block_reach_limit(GameMode::Creative),
            "{gap} should sit between the two limits"
        );

        assert!(can_reach_block(feet, false, GameMode::Creative, 7, 65, 0));
        assert!(!can_reach_block(feet, false, GameMode::Survival, 7, 65, 0));
    }

    #[test]
    fn sneaking_lowers_the_measuring_point() {
        assert!(EntityBounds::player(true).height < EntityBounds::player(false).height);

        let feet = standing_at(0.5, 64.0, 0.5);
        let standing = dist_sq_to_block(
            (0.5, 64.0 + EntityBounds::player(false).height, 0.5),
            5,
            60,
            0,
        );
        let sneaking = dist_sq_to_block(
            (0.5, 64.0 + EntityBounds::player(true).height, 0.5),
            5,
            60,
            0,
        );
        assert!(sneaking < standing, "{sneaking} should be under {standing}");
        let _ = feet;
    }

    #[test]
    fn a_non_finite_position_can_reach_nothing() {
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(!can_reach_block(
                (bad, 64.0, 0.5),
                false,
                GameMode::Creative,
                0,
                63,
                0
            ));
            assert!(!can_reach_block(
                (0.5, bad, 0.5),
                false,
                GameMode::Creative,
                0,
                63,
                0
            ));
        }
    }

    #[test]
    fn range_does_not_stretch_along_diagonals() {
        let feet = standing_at(0.5, 64.0, 0.5);
        let limit = block_reach_limit(GameMode::Survival);

        for (bx, bz) in [(4, 4), (5, 5), (-4, 4), (4, -4)] {
            let eye = (0.5, 64.0 + EntityBounds::player(false).height, 0.5);
            let d = dist_sq_to_block(eye, bx, 65, bz).sqrt();
            assert_eq!(
                can_reach_block(feet, false, GameMode::Survival, bx, 65, bz),
                d <= limit,
                "({bx}, {bz}) at distance {d} against limit {limit}"
            );
        }
    }

    #[test]
    fn the_limits_leave_room_for_the_clients_own_range() {
        assert!(
            block_reach_limit(GameMode::Survival) > SURVIVAL_BLOCK_REACH,
            "no tolerance over the client's limit means lag refuses real players"
        );
        assert!(block_reach_limit(GameMode::Creative) > CREATIVE_BLOCK_REACH);
        assert_eq!(LATENCY_TOLERANCE, 1.0);
    }
}
