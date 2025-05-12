use crate::protocol::properties::PropertyMap;
use super::living::Living;

pub struct Player {
    pub base: Living
}

pub struct UUID([u8; 16]);
pub struct GameProfile {
    uuid: UUID,
    name: String,
    properties: PropertyMap,
    legacy: bool
}