use coral_types::GameMode;
use uuid::Uuid;

use coral_protocol::auth::ProfileProperty;

use crate::effects::ActiveEffect;

#[derive(Debug, Clone)]
pub struct Player {
    pub entity_id: i32,
    pub uuid: Uuid,
    pub username: String,
    pub properties: Vec<ProfileProperty>,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub gamemode: GameMode,
    pub held_slot: u8,
    pub held_item_id: i16,
    pub latency_ms: u32,
    pub health: f32,
    pub food: i32,
    pub food_saturation: f32,
    pub is_dead: bool,
    pub is_sneaking: bool,
    pub is_sprinting: bool,
    pub skin_parts: u8,
    pub no_damage_ticks: i32,
    pub helmet: i16,
    pub chestplate: i16,
    pub leggings: i16,
    pub boots: i16,
    pub active_effects: Vec<ActiveEffect>,
}

impl Player {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        entity_id: i32,
        uuid: Uuid,
        username: String,
        properties: Vec<ProfileProperty>,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        gamemode: GameMode,
        health: f32,
        food: i32,
        food_saturation: f32,
    ) -> Self {
        Self {
            entity_id,
            uuid,
            username,
            properties,
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground: true,
            gamemode,
            held_slot: 0,
            held_item_id: -1,
            latency_ms: 0,
            health,
            food,
            food_saturation,
            is_dead: false,
            is_sneaking: false,
            is_sprinting: false,
            skin_parts: 0x7F,
            no_damage_ticks: 0,
            helmet: -1,
            chestplate: -1,
            leggings: -1,
            boots: -1,
            active_effects: vec![],
        }
    }

    pub fn entity_flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.is_sneaking {
            flags |= 0x02;
        }
        if self.is_sprinting {
            flags |= 0x08;
        }
        flags
    }
}
