use std::{
    collections::HashMap,
    sync::atomic::{AtomicI32, Ordering::Relaxed},
};

use coral_types::GameMode;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{effects::ActiveEffect, player::Player};

static ENTITY_ID_COUNTER: AtomicI32 = AtomicI32::new(1);

pub fn next_entity_id() -> i32 {
    ENTITY_ID_COUNTER.fetch_add(1, Relaxed)
}

#[derive(Debug, Default)]
pub struct Roster {
    by_uuid: HashMap<Uuid, Player>,
    by_entity_id: HashMap<i32, Uuid>,
}
impl Roster {
    fn insert(&mut self, player: Player) {
        let (uuid, entity_id) = (player.uuid, player.entity_id);
        if let Some(previous) = self.by_uuid.insert(uuid, player) {
            self.by_entity_id.remove(&previous.entity_id);
        }
        self.by_entity_id.insert(entity_id, uuid);
    }

    fn remove(&mut self, uuid: &Uuid) {
        if let Some(player) = self.by_uuid.remove(uuid) {
            self.by_entity_id.remove(&player.entity_id);
        }
    }
}

#[derive(Debug)]
pub struct PlayerRegistry {
    roster: RwLock<Roster>,
}
impl PlayerRegistry {
    pub fn new() -> Self {
        Self {
            roster: RwLock::new(Roster::default()),
        }
    }

    pub async fn is_empty(&self) -> bool {
        self.roster.read().await.by_uuid.is_empty()
    }

    pub async fn add(&self, player: Player) {
        self.roster.write().await.insert(player);
    }

    pub async fn remove(&self, uuid: &Uuid) {
        self.roster.write().await.remove(uuid);
    }

    pub async fn get_all(&self) -> Vec<Player> {
        self.roster.read().await.by_uuid.values().cloned().collect()
    }

    pub async fn get_online_count(&self) -> u32 {
        self.roster.read().await.by_uuid.len() as u32
    }

    pub async fn tick(&self) {
        for p in self.roster.write().await.by_uuid.values_mut() {
            if p.no_damage_ticks > 0 {
                p.no_damage_ticks -= 1;
            }
        }
    }

    pub async fn get(&self, uuid: &Uuid) -> Option<Player> {
        self.roster.read().await.by_uuid.get(uuid).cloned()
    }
    pub async fn get_by_entity_id(&self, entity_id: i32) -> Option<Player> {
        let roster = self.roster.read().await;
        let uuid = roster.by_entity_id.get(&entity_id)?;
        roster.by_uuid.get(uuid).cloned()
    }

    pub async fn update_gamemode(&self, uuid: &Uuid, gamemode: GameMode) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.gamemode = gamemode;
        }
    }

    pub async fn update_on_fire(&self, uuid: &Uuid, on_fire: bool) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.on_fire = on_fire;
        }
    }

    pub async fn update_held_slot(&self, uuid: &Uuid, held_slot: u8) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.held_slot = held_slot;
        }
    }

    pub async fn update_health(&self, uuid: &Uuid, health: f32, food: i32, food_saturation: f32) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.health = health;
            player.food = food;
            player.food_saturation = food_saturation;
            player.is_dead = health <= 0.0
        }
    }

    pub async fn update_held_item(&self, uuid: &Uuid, item_id: i16) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.held_item_id = item_id;
        }
    }

    pub async fn update_armor(&self, uuid: &Uuid, helmet: i16, chest: i16, legs: i16, boots: i16) {
        if let Some(p) = self.roster.write().await.by_uuid.get_mut(uuid) {
            p.helmet = helmet;
            p.chestplate = chest;
            p.leggings = legs;
            p.boots = boots
        }
    }

    pub async fn get_armor(&self, uuid: &Uuid) -> (i16, i16, i16, i16) {
        self.roster
            .read()
            .await
            .by_uuid
            .get(uuid)
            .map(|p| (p.helmet, p.chestplate, p.leggings, p.boots))
            .unwrap_or((-1, -1, -1, -1))
    }

    pub async fn update_latency(&self, uuid: &Uuid, latency_ms: i32) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.latency_ms = latency_ms;
        }
    }

    pub async fn update_sprinting(&self, uuid: &Uuid, is_sprinting: bool) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.is_sprinting = is_sprinting;
        }
    }
    pub async fn update_sneaking(&self, uuid: &Uuid, is_sneaking: bool) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.is_sneaking = is_sneaking;
        }
    }

    pub async fn update_air_tick(&self, uuid: &Uuid, air_tick: i16) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.air_tick = air_tick;
        }
    }

    pub async fn update_skin_parts(&self, uuid: &Uuid, skin_parts: u8) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.skin_parts = skin_parts;
        }
    }

    pub async fn update_no_damage_ticks(&self, uuid: &Uuid, no_damage_ticks: i32) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.no_damage_ticks = no_damage_ticks;
        }
    }

    pub async fn update_effects(&self, uuid: &Uuid, effects: Vec<ActiveEffect>) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.active_effects = effects;
        }
    }

    pub async fn get_effects(&self, uuid: &Uuid) -> Vec<ActiveEffect> {
        self.roster
            .read()
            .await
            .by_uuid
            .get(uuid)
            .map(|p| p.active_effects.clone())
            .unwrap_or_default()
    }

    pub async fn update_sleeping(&self, uuid: &Uuid, sleeping: bool) {
        if let Some(p) = self.roster.write().await.by_uuid.get_mut(uuid) {
            p.is_sleeping = sleeping;
        }
    }

    pub async fn count_sleeping(&self) -> usize {
        self.roster
            .read()
            .await
            .by_uuid
            .values()
            .filter(|p| p.is_sleeping)
            .count()
    }

    pub async fn clear_all_sleeping(&self) {
        for p in self.roster.write().await.by_uuid.values_mut() {
            p.is_sleeping = false;
        }
    }

    pub async fn update_velocity(&self, uuid: &Uuid, velocity: (f64, f64, f64)) {
        if let Some(p) = self.roster.write().await.by_uuid.get_mut(uuid) {
            p.velocity = velocity;
        }
    }

    pub async fn get_velocity(&self, uuid: &Uuid) -> (f64, f64, f64) {
        self.roster
            .read()
            .await
            .by_uuid
            .get(uuid)
            .map(|p| p.velocity)
            .unwrap_or((0.0, 0.0, 0.0))
    }

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
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
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

#[cfg(test)]
mod entity_id_index_tests {
    use super::*;

    fn player(entity_id: i32, uuid: Uuid) -> Player {
        Player::new(
            entity_id,
            uuid,
            format!("p{entity_id}"),
            vec![],
            0.0,
            64.0,
            0.0,
            0.0,
            0.0,
            GameMode::Survival,
            20.0,
            20,
            5.0,
        )
    }

    async fn registry_with(players: &[(i32, Uuid)]) -> PlayerRegistry {
        let registry = PlayerRegistry::new();
        for &(eid, uuid) in players {
            registry.add(player(eid, uuid)).await;
        }
        registry
    }

    #[tokio::test]
    async fn every_player_is_findable_by_entity_id() {
        let ids: Vec<(i32, Uuid)> = (1..=20).map(|i| (i, Uuid::new_v4())).collect();
        let registry = registry_with(&ids).await;

        for (eid, uuid) in &ids {
            let found = registry.get_by_entity_id(*eid).await;
            assert_eq!(found.map(|p| p.uuid), Some(*uuid), "entity id {eid}");
        }
    }

    #[tokio::test]
    async fn an_unknown_entity_id_is_none() {
        let registry = registry_with(&[(1, Uuid::new_v4())]).await;
        assert!(registry.get_by_entity_id(999).await.is_none());
    }

    #[tokio::test]
    async fn an_empty_registry_is_none() {
        assert!(PlayerRegistry::new().get_by_entity_id(1).await.is_none());
    }

    #[tokio::test]
    async fn removing_a_player_clears_their_entity_id() {
        let uuid = Uuid::new_v4();
        let registry = registry_with(&[(7, uuid), (8, Uuid::new_v4())]).await;

        registry.remove(&uuid).await;

        assert!(
            registry.get_by_entity_id(7).await.is_none(),
            "a removed player is still in the index"
        );
        assert!(
            registry.get_by_entity_id(8).await.is_some(),
            "wrong one went"
        );
    }

    #[tokio::test]
    async fn a_reconnect_retires_the_old_entity_id() {
        let uuid = Uuid::new_v4();
        let registry = registry_with(&[(11, uuid)]).await;
        registry.add(player(12, uuid)).await;

        assert!(registry.get_by_entity_id(11).await.is_none(), "stale id");
        assert_eq!(
            registry.get_by_entity_id(12).await.map(|p| p.uuid),
            Some(uuid)
        );
        assert_eq!(registry.get_online_count().await, 1);
    }

    #[tokio::test]
    async fn the_index_matches_a_scan_after_a_churn_of_joins_and_leaves() {
        let registry = PlayerRegistry::new();
        let mut live: Vec<(i32, Uuid)> = Vec::new();

        for i in 1..=40i32 {
            let uuid = Uuid::new_v4();
            registry.add(player(i, uuid)).await;
            live.push((i, uuid));
            if i % 3 == 0 {
                let (_, gone) = live.remove(i as usize % live.len());
                registry.remove(&gone).await;
            }
        }

        let all = registry.get_all().await;
        for i in 1..=60i32 {
            // what a scan of the players would have said
            let expected = all.iter().find(|p| p.entity_id == i).map(|p| p.uuid);
            let actual = registry.get_by_entity_id(i).await.map(|p| p.uuid);
            assert_eq!(actual, expected, "entity id {i} disagreed with a scan");
        }
    }
}
