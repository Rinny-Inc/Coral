use uuid::Uuid;

pub type DamageEvent = (Uuid, f32, i32, f32, i32);
pub type PingUpdate = (Uuid, i32);
pub type BlockUpdate = (i32, i32, i32, i32, u8);
pub type BreakAnimation = (i32, i32, i32, i32, u8);
pub type MetadataUpdate = (i32, u8, u8);
pub type ItemDrop = (i32, f64, f64, f64, i16, u8, i16);
pub type DespawnEntity = Vec<i32>;
pub type ItemInfo = (i32, f64, f64, f64, i16, u8, i16);
pub type ItemPickup = (i32, Uuid, i32);
pub type TimeUpdate = (i64, i64);
pub type EquipmentUpdate = (i32, i16, i16, u8, i16);
pub type SoundEffect = (String, f64, f64, f64, f32, u8);
pub type ParticleEffect = (i32, i32, f32, f32, f32, f32, f32, f32, f32, i32);
pub type ProjectileMove = (i32, f64, f64, f64);
pub type SplashEffect = (Uuid, u8, u8, i32);
pub type XpOrbSpawn = (i32, f64, f64, f64, i32);
pub type XpOrbMove = (i32, f64, f64, f64);
pub type XpPickup = (Uuid, i32);
pub type BedUpdate = (i32, i32, i32, i32);
pub type PrivateMessage = (String, String, String);

#[derive(Debug, Clone, PartialEq)]
pub enum ToolKind {
    Pickaxe,
    Axe,
    Shovel,
    Sword,
    None,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ToolMaterial {
    Wood,
    Stone,
    Iron,
    Gold,
    Diamond,
    Any,
}

pub type GamemodeUpdate = (Uuid, GameMode);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GameMode {
    Survival,
    Creative,
    Adventure,
    Spectator,
}
impl TryFrom<u8> for GameMode {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Survival),
            1 => Ok(Self::Creative),
            2 => Ok(Self::Adventure),
            3 => Ok(Self::Spectator),
            _ => Err(value),
        }
    }
}
impl From<GameMode> for u8 {
    fn from(value: GameMode) -> Self {
        value as u8
    }
}

#[inline]
pub fn dist_xz(x1: f64, z1: f64, x2: f64, z2: f64) -> f64 {
    dist_sq_xz(x1, z1, x2, z2).sqrt()
}

#[inline]
pub fn dist_sq_xz(x1: f64, z1: f64, x2: f64, z2: f64) -> f64 {
    let dx = x1 - x2;
    let dz = z1 - z2;
    dx * dx + dz * dz
}

#[inline]
pub fn dist3(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> f64 {
    dist_sq3(x1, y1, z1, x2, y2, z2).sqrt()
}

#[inline]
pub fn dist_sq3(x1: f64, y1: f64, z1: f64, x2: f64, y2: f64, z2: f64) -> f64 {
    let dx = x1 - x2;
    let dy = y1 - y2;
    let dz = z1 - z2;
    dx * dx + dy * dy + dz * dz
}

#[inline]
pub fn look_direction(yaw: f32, pitch: f32) -> (f64, f64, f64) {
    let yaw_rad = yaw * std::f32::consts::PI / 180.0;
    let pitch_rad = pitch * std::f32::consts::PI / 180.0;
    let dx = (-yaw_rad.sin() * pitch_rad.cos()) as f64;
    let dy = (-pitch_rad.sin()) as f64;
    let dz = (yaw_rad.cos() * pitch_rad.cos()) as f64;
    (dx, dy, dz)
}

pub fn offline_uuid(username: &str) -> Uuid {
    Uuid::new_v3(
        &Uuid::NAMESPACE_DNS,
        format!("OfflinePlayer:{}", username).as_bytes(),
    )
}
