use std::{
    collections::HashMap,
    sync::atomic::{AtomicI32, Ordering::Relaxed},
};

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::server::player::Player;

static ENTITY_ID_COUNTER: AtomicI32 = AtomicI32::new(1);

pub fn next_entity_id() -> i32 {
    ENTITY_ID_COUNTER.fetch_add(1, Relaxed)
}

#[derive(Debug)]
pub struct PlayerRegistry {
    pub players: RwLock<HashMap<Uuid, Player>>,
}
impl PlayerRegistry {
    pub fn new() -> Self {
        Self {
            players: RwLock::new(HashMap::new()),
        }
    }

    pub async fn add(&self, player: Player) {
        self.players.write().await.insert(player.uuid, player);
    }

    pub async fn remove(&self, uuid: &Uuid) {
        self.players.write().await.remove(uuid);
    }

    pub async fn get_all(&self) -> Vec<Player> {
        self.players.read().await.values().cloned().collect()
    }

    pub async fn get_online_count(&self) -> u32 {
        self.players.read().await.len() as u32
    }

    pub async fn get(&self, uuid: &Uuid) -> Option<Player> {
        self.players.read().await.get(&uuid).cloned()
    }

    pub async fn update_gamemode(&self, uuid: Uuid, gamemode: u8) {
        if let Some(player) = self.players.write().await.get_mut(&uuid) {
            player.gamemode = gamemode;
        }
    }

    pub async fn update_held_slot(&self, uuid: Uuid, held_slot: u8) {
        if let Some(player) = self.players.write().await.get_mut(&uuid) {
            player.held_slot = held_slot;
        }
    }

    pub async fn update_held_item(&self, uuid: Uuid, item_id: i16) {
        if let Some(player) = self.players.write().await.get_mut(&uuid) {
            player.held_item_id = item_id;
        }
    }

    // TODO: could prolly have less argument by doing structs
    #[allow(clippy::too_many_arguments)]
    pub async fn update_position(
        &self,
        uuid: &Uuid,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    ) {
        if let Some(player) = self.players.write().await.get_mut(uuid) {
            player.x = x;
            player.y = y;
            player.z = z;
            player.yaw = yaw;
            player.pitch = pitch;
            player.on_ground = on_ground;
        }
    }
}
impl Default for PlayerRegistry {
    fn default() -> Self {
        Self::new()
    }
}
