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
    fn the_object_types_are_the_1_8_ids() {
        assert_eq!(ProjectileKind::Arrow.entity_id(), 60);
        assert_eq!(ProjectileKind::FishingHook.entity_id(), 90);
        assert_eq!(ProjectileKind::SplashPotion(0).entity_id(), 73);
    }
}
