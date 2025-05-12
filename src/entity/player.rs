use crate::protocol::properties::PropertyMap;
use super::living::Living;
use uuid::Uuid;

pub struct Player {
    pub base: Living
}

pub struct GameProfile {
    uuid: Uuid,
    name: String,
    properties: PropertyMap,
    legacy: bool
}