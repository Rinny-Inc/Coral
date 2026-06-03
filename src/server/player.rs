use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Player {
    pub entity_id: i32,
    pub uuid: Uuid,
    pub username: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub gamemode: u8,
    pub held_slot: u8,
    pub held_item_id: i16,
}

impl Player {
    pub fn new(entity_id: i32, uuid: Uuid, username: String) -> Self {
        Self {
            entity_id,
            uuid,
            username,
            x: 0.5,
            y: 5.0,
            z: 0.5,
            yaw: 90.0,
            pitch: 0.0,
            on_ground: true,
            gamemode: 0,
            held_slot: 0,
            held_item_id: -1,
        }
    }
}
