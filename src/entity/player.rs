use crate::protocol::properties::PropertyMap;
use super::{living::{Living, LivingTrait}, EntityTrait};
use uuid::Uuid;

pub struct Player {
    id: u128,
    health: f32,
    name: String
}

impl EntityTrait for Player {
    fn id(&self) -> u128 {
        self.id
    }
}

impl LivingTrait for Player {
    fn health(&self) -> f32 {
        self.health
    }

    fn name(&self) -> &str {
        &self.name
    }
}


pub struct GameProfile {
    uuid: Uuid,
    name: String,
    properties: PropertyMap,
    legacy: bool
}