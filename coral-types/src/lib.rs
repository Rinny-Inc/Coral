use uuid::Uuid;

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
