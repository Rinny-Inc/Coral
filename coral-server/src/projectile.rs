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
