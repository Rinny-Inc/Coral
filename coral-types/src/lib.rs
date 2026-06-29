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
