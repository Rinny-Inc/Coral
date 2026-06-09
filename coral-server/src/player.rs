use uuid::Uuid;

use coral_protocol::auth::ProfileProperty;

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
    pub gamemode: u8,
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
}

impl Player {
    pub fn new(
        entity_id: i32,
        uuid: Uuid,
        username: String,
        properties: Vec<ProfileProperty>,
    ) -> Self {
        Self {
            entity_id,
            uuid,
            username,
            properties,
            x: 0.5,
            y: 5.0,
            z: 0.5,
            yaw: 90.0,
            pitch: 0.0,
            on_ground: true,
            gamemode: 0,
            held_slot: 0,
            held_item_id: -1,
            latency_ms: 0,
            health: 20.0,
            food: 20,
            food_saturation: 5.0,
            is_dead: false,
            is_sneaking: false,
            is_sprinting: false,
            skin_parts: 0x7F,
            no_damage_ticks: 0,
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
