#[derive(Debug, Clone)]
pub struct Projectile {
    pub entity_id: i32,
    pub owner_entity_id: i32,
    pub kind: ProjectileKind,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub ticks_alive: u32,
    pub left_owner: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectileKind {
    Arrow,
    FishingHook,
    SplashPotion(i16),
}
impl ProjectileKind {
    pub fn entity_id(&self) -> u8 {
        match self {
            Self::Arrow => 60,
            Self::FishingHook => 90,
            Self::SplashPotion(_) => 73,
        }
    }

    pub fn gravity(&self) -> f64 {
        match self {
            Self::Arrow | Self::SplashPotion(_) => 0.05,
            Self::FishingHook => 0.03,
        }
    }
}

pub fn rotation(vx: f64, vy: f64, vz: f64) -> (f32, f32) {
    let horizontal = (vx * vx + vz * vz).sqrt();
    let yaw = vx.atan2(vz).to_degrees() as f32;
    let pitch = vy.atan2(horizontal).to_degrees() as f32;
    (yaw, pitch)
}

pub fn launch_velocity(
    aim: (f64, f64, f64),
    owner_velocity: (f64, f64, f64),
    speed: f64,
) -> (f64, f64, f64) {
    (
        aim.0 * speed + owner_velocity.0,
        aim.1 * speed + owner_velocity.1,
        aim.2 * speed + owner_velocity.2,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn magnitude((x, y, z): (f64, f64, f64)) -> f64 {
        (x * x + y * y + z * z).sqrt()
    }

    #[test]
    fn a_standing_player_casts_at_the_configured_speed() {
        let v = launch_velocity((0.0, 0.0, 1.0), (0.0, 0.0, 0.0), 1.5);
        assert_eq!(v, (0.0, 0.0, 1.5));

        let aim = (0.6, -0.8, 0.0);
        assert!((magnitude(launch_velocity(aim, (0.0, 0.0, 0.0), 1.5)) - 1.5).abs() < 1e-9);
    }

    #[test]
    fn owner_motion_is_added_unscaled() {
        let v = launch_velocity((0.0, 0.0, 1.0), (0.3, 0.0, 0.0), 1.5);
        assert_eq!(v, (0.3, 0.0, 1.5));

        let scaled_by_mistake = (0.3 * 1.5, 0.0, 1.0);
        assert_ne!(v, scaled_by_mistake);
    }

    #[test]
    fn aiming_straight_down_sends_it_straight_down() {
        let v = launch_velocity((0.0, -1.0, 0.0), (0.0, 0.0, 0.0), 1.5);
        assert_eq!(v, (0.0, -1.5, 0.0));
    }

    #[test]
    fn heading_south_is_zero_yaw() {
        let (yaw, pitch) = rotation(0.0, 0.0, 1.0);
        assert!(yaw.abs() < 1e-4, "yaw was {yaw}");
        assert!(pitch.abs() < 1e-4, "pitch was {pitch}");
    }

    #[test]
    fn the_cardinal_directions_have_the_expected_yaw() {
        for (vx, vz, expected) in [
            (0.0, 1.0, 0.0),    // south
            (1.0, 0.0, 90.0),   // east
            (0.0, -1.0, 180.0), // north
            (-1.0, 0.0, -90.0), // west
        ] {
            let (yaw, _) = rotation(vx, 0.0, vz);
            assert!(
                (yaw - expected).abs() < 1e-3,
                "({vx}, {vz}) gave yaw {yaw}, expected {expected}"
            );
        }
    }

    #[test]
    fn falling_gives_a_negative_pitch_and_rising_a_positive_one() {
        let (_, falling) = rotation(0.0, -1.0, 1.0);
        let (_, rising) = rotation(0.0, 1.0, 1.0);

        assert!((falling + 45.0).abs() < 1e-3, "falling pitch {falling}");
        assert!((rising - 45.0).abs() < 1e-3, "rising pitch {rising}");
    }

    #[test]
    fn straight_down_is_minus_ninety_degrees_of_pitch() {
        let (_, pitch) = rotation(0.0, -1.0, 0.0);
        assert!((pitch + 90.0).abs() < 1e-3, "pitch {pitch}");

        let (_, up) = rotation(0.0, 1.0, 0.0);
        assert!((up - 90.0).abs() < 1e-3, "pitch {up}");
    }

    #[test]
    fn a_projectile_rotation_negates_the_shooters() {
        fn aim(yaw_deg: f64, pitch_deg: f64) -> (f64, f64, f64) {
            let (yaw, pitch) = (yaw_deg.to_radians(), pitch_deg.to_radians());
            (
                -yaw.sin() * pitch.cos(),
                -pitch.sin(),
                yaw.cos() * pitch.cos(),
            )
        }

        for (yaw, pitch) in [
            (0.0, 0.0),
            (45.0, 30.0),
            (-120.0, -15.0),
            (170.0, 60.0),
            (-30.0, 0.0),
        ] {
            let (vx, vy, vz) = aim(yaw, pitch);
            let (got_yaw, got_pitch) = rotation(vx, vy, vz);

            assert!(
                (got_yaw as f64 + yaw).abs() < 1e-2,
                "shooter yaw {yaw} gave projectile yaw {got_yaw}, expected {}",
                -yaw
            );
            assert!(
                (got_pitch as f64 + pitch).abs() < 1e-2,
                "shooter pitch {pitch} gave projectile pitch {got_pitch}, expected {}",
                -pitch
            );
        }
    }

    #[test]
    fn zero_velocity_gives_a_finite_rotation() {
        let (yaw, pitch) = rotation(0.0, 0.0, 0.0);
        assert!(yaw.is_finite() && pitch.is_finite(), "{yaw}, {pitch}");
        assert_eq!((yaw, pitch), (0.0, 0.0));
    }

    #[test]
    fn rotation_depends_on_direction_not_speed() {
        let fast = rotation(0.6, -0.3, 0.8);
        for scale in [0.5, 0.1, 0.01] {
            let slow = rotation(0.6 * scale, -0.3 * scale, 0.8 * scale);
            assert!(
                (fast.0 - slow.0).abs() < 1e-3 && (fast.1 - slow.1).abs() < 1e-3,
                "scale {scale} moved the rotation from {fast:?} to {slow:?}"
            );
        }
    }

    #[test]
    fn an_arcing_arrow_pitches_down_as_it_falls() {
        let mut vy = 0.0f64;
        let (_, level) = rotation(0.0, vy, 1.0);
        assert!(level.abs() < 1e-4, "it starts level");

        let mut last = level;
        for _ in 0..5 {
            vy -= ProjectileKind::Arrow.gravity();
            let (_, pitch) = rotation(0.0, vy, 1.0);
            assert!(
                pitch < last,
                "pitch should keep decreasing as it falls: {pitch} after {last}"
            );
            last = pitch;
        }
    }

    #[test]
    fn the_object_types_are_the_1_8_ids() {
        assert_eq!(ProjectileKind::Arrow.entity_id(), 60);
        assert_eq!(ProjectileKind::FishingHook.entity_id(), 90);
        assert_eq!(ProjectileKind::SplashPotion(0).entity_id(), 73);
    }
}
