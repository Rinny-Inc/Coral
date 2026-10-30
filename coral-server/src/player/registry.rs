use std::{
    collections::HashMap,
    sync::atomic::{AtomicI32, Ordering::Relaxed},
};

use coral_types::GameMode;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{effects::ActiveEffect, items::drops::chunk_of, player::Player};

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NearbyPlayer {
    pub uuid: Uuid,
    pub entity_id: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub is_dead: bool,
}
impl From<&Player> for NearbyPlayer {
    fn from(p: &Player) -> Self {
        Self {
            uuid: p.uuid,
            entity_id: p.entity_id,
            x: p.x,
            y: p.y,
            z: p.z,
            is_dead: p.is_dead,
        }
    }
}

const BUCKET_THRESHOLD: usize = 32;

#[derive(Debug, Default)]
pub struct PlayerPositions {
    all: Vec<NearbyPlayer>,
    by_chunk: Option<HashMap<(i32, i32), Vec<NearbyPlayer>>>,
}

/// Iterator over the players a query reaches, from whichever representation the snapshot chose
pub enum NearIter<'a> {
    /// Every player, filtered by the caller's own exact test
    All(std::slice::Iter<'a, NearbyPlayer>),
    /// Only the chunks the radius touches
    Buckets {
        by_chunk: &'a HashMap<(i32, i32), Vec<NearbyPlayer>>,
        min_cx: i32,
        min_cz: i32,
        width: i64,
        total: i64,
        visited: i64,
        current: std::slice::Iter<'a, NearbyPlayer>,
    },
}
impl NearIter<'_> {
    fn chunk_at(min_cx: i32, min_cz: i32, width: i64, step: i64) -> (i32, i32) {
        (
            min_cx + (step / width) as i32,
            min_cz + (step % width) as i32,
        )
    }
}
impl<'a> Iterator for NearIter<'a> {
    type Item = &'a NearbyPlayer;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::All(iter) => iter.next(),
            Self::Buckets {
                by_chunk,
                min_cx,
                min_cz,
                width,
                total,
                visited,
                current,
            } => loop {
                if let Some(player) = current.next() {
                    return Some(player);
                }
                if *visited >= *total {
                    return None;
                }
                let key = Self::chunk_at(*min_cx, *min_cz, *width, *visited);
                *visited += 1;
                *current = by_chunk
                    .get(&key)
                    .map(|v| v.iter())
                    .unwrap_or_else(|| [].iter());
            },
        }
    }
}

impl PlayerPositions {
    #[inline]
    pub fn candidates_near(&self, x: f64, z: f64, radius: f64) -> NearIter<'_> {
        let radius = radius.max(0.0);
        let Some(by_chunk) = &self.by_chunk else {
            return NearIter::All(self.all.iter());
        };
        let (min_cx, min_cz) = chunk_of(x - radius, z - radius);
        let (max_cx, max_cz) = chunk_of(x + radius, z + radius);
        let width = i64::from(max_cz - min_cz) + 1;
        let height = i64::from(max_cx - min_cx) + 1;
        NearIter::Buckets {
            by_chunk,
            min_cx,
            min_cz,
            width,
            total: width * height,
            visited: 0,
            current: [].iter(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.all.is_empty()
    }

    pub fn len(&self) -> usize {
        self.all.len()
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

    pub async fn update_experience(&self, uuid: &Uuid, xp_total: i32, xp_level: i32) {
        if let Some(player) = self.roster.write().await.by_uuid.get_mut(uuid) {
            player.xp_total = xp_total;
            player.xp_level = xp_level;
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

    pub async fn position_snapshot(&self) -> PlayerPositions {
        let roster = self.roster.read().await;
        let all: Vec<NearbyPlayer> = roster.by_uuid.values().map(NearbyPlayer::from).collect();

        let by_chunk = (all.len() >= BUCKET_THRESHOLD).then(|| {
            let mut buckets: HashMap<(i32, i32), Vec<NearbyPlayer>> = HashMap::new();
            for player in &all {
                buckets
                    .entry(chunk_of(player.x, player.z))
                    .or_default()
                    .push(*player);
            }
            buckets
        });

        PlayerPositions { all, by_chunk }
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
            0,
            0,
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
    async fn experience_is_visible_through_the_registry() {
        let uuid = Uuid::new_v4();
        let registry = registry_with(&[(1, uuid)]).await;

        assert_eq!(registry.get(&uuid).await.map(|p| p.xp_level), Some(0));

        registry.update_experience(&uuid, 394, 15).await;

        let p = registry.get(&uuid).await.expect("still online");
        assert_eq!((p.xp_total, p.xp_level), (394, 15));
        let by_eid = registry.get_by_entity_id(1).await.expect("tracked");
        assert_eq!((by_eid.xp_total, by_eid.xp_level), (394, 15));
    }

    #[tokio::test]
    async fn clearing_experience_is_visible_too() {
        let uuid = Uuid::new_v4();
        let registry = registry_with(&[(1, uuid)]).await;
        registry.update_experience(&uuid, 394, 15).await;

        registry.update_experience(&uuid, 0, 0).await;

        let p = registry.get(&uuid).await.expect("still online");
        assert_eq!((p.xp_total, p.xp_level), (0, 0));
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

#[cfg(test)]
mod position_snapshot_tests {
    use super::*;

    fn at(entity_id: i32, uuid: Uuid, x: f64, z: f64) -> Player {
        Player::new(
            entity_id,
            uuid,
            format!("p{entity_id}"),
            vec![],
            x,
            64.0,
            z,
            0.0,
            0.0,
            GameMode::Survival,
            20.0,
            20,
            5.0,
            0,
            0,
        )
    }

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn coord(&mut self) -> f64 {
            (self.next() % 4000) as f64 - 2000.0
        }
    }

    async fn pad_past_threshold(r: &PlayerRegistry) {
        for i in 0..BUCKET_THRESHOLD as i32 {
            r.add(at(
                10_000 + i,
                Uuid::new_v4(),
                50_000.0 + i as f64 * 64.0,
                50_000.0,
            ))
            .await;
        }
    }

    #[tokio::test]
    async fn an_empty_registry_finds_nobody() {
        let r = PlayerRegistry::new();
        let snapshot = r.position_snapshot().await;
        assert!(snapshot.is_empty());
        assert_eq!(snapshot.candidates_near(0.0, 0.0, 16.0).count(), 0);
    }

    #[tokio::test]
    async fn a_player_in_the_same_chunk_is_a_candidate() {
        let r = PlayerRegistry::new();
        let uuid = Uuid::new_v4();
        r.add(at(1, uuid, 8.0, 8.0)).await;

        let snapshot = r.position_snapshot().await;
        let near: Vec<_> = snapshot.candidates_near(8.0, 8.0, 1.0).collect();
        assert_eq!(near.len(), 1);
        assert_eq!(near[0].uuid, uuid);
    }

    #[tokio::test]
    async fn below_the_threshold_everyone_is_a_candidate() {
        let r = PlayerRegistry::new();
        r.add(at(1, Uuid::new_v4(), 1000.0, 1000.0)).await;

        let snapshot = r.position_snapshot().await;
        assert!(snapshot.by_chunk.is_none(), "bucketed too eagerly");
        assert_eq!(snapshot.candidates_near(0.0, 0.0, 1.0).count(), 1);
    }

    #[tokio::test]
    async fn above_the_threshold_distant_players_are_skipped() {
        let r = PlayerRegistry::new();
        pad_past_threshold(&r).await;
        r.add(at(1, Uuid::new_v4(), 1000.0, 1000.0)).await;

        let snapshot = r.position_snapshot().await;
        assert!(snapshot.by_chunk.is_some(), "should have bucketed");
        assert_eq!(snapshot.candidates_near(0.0, 0.0, 1.0).count(), 0);
        assert_eq!(snapshot.candidates_near(1000.0, 1000.0, 1.0).count(), 1);
    }

    #[tokio::test]
    async fn the_snapshot_follows_a_player_who_walks_away() {
        let r = PlayerRegistry::new();
        pad_past_threshold(&r).await;
        let uuid = Uuid::new_v4();
        r.add(at(1, uuid, 8.0, 8.0)).await;

        r.update_position(&uuid, 500.0, 64.0, 500.0, 0.0, 0.0, true)
            .await;

        let snapshot = r.position_snapshot().await;
        assert_eq!(
            snapshot.candidates_near(8.0, 8.0, 1.0).count(),
            0,
            "still listed where they used to be"
        );
        assert_eq!(snapshot.candidates_near(500.0, 500.0, 1.0).count(), 1);
    }

    #[tokio::test]
    async fn a_player_who_leaves_is_gone_from_the_next_snapshot() {
        let r = PlayerRegistry::new();
        let uuid = Uuid::new_v4();
        r.add(at(1, uuid, 8.0, 8.0)).await;
        r.remove(&uuid).await;

        assert!(r.position_snapshot().await.is_empty());
    }

    #[tokio::test]
    async fn a_player_appears_exactly_once() {
        let r = PlayerRegistry::new();
        let uuid = Uuid::new_v4();
        r.add(at(1, uuid, 1.0, 1.0)).await;
        for step in 2..40 {
            r.update_position(&uuid, step as f64, 64.0, 1.0, 0.0, 0.0, true)
                .await;
        }
        let snapshot = r.position_snapshot().await;
        assert_eq!(snapshot.len(), 1);
        assert_eq!(
            snapshot.candidates_near(-1000.0, -1000.0, 4000.0).count(),
            1
        );
    }

    #[tokio::test]
    async fn a_radius_spanning_chunks_reaches_all_of_them() {
        let r = PlayerRegistry::new();
        pad_past_threshold(&r).await;
        for (i, (cx, cz)) in (-1..=1)
            .flat_map(|x| (-1..=1).map(move |z| (x, z)))
            .enumerate()
        {
            let (x, z) = ((cx * 16 + 8) as f64, (cz * 16 + 8) as f64);
            r.add(at(i as i32, Uuid::new_v4(), x, z)).await;
        }

        let snapshot = r.position_snapshot().await;
        assert_eq!(
            snapshot.candidates_near(8.0, 8.0, 1.0).count(),
            1,
            "own chunk"
        );
        assert_eq!(
            snapshot.candidates_near(8.0, 8.0, 24.0).count(),
            9,
            "all nine"
        );
    }

    #[tokio::test]
    async fn it_never_misses_anyone_a_full_scan_would_find() {
        for population in [8usize, 80] {
            let r = PlayerRegistry::new();
            let mut rng = Rng(0x5EED + population as u64);

            let mut uuids = Vec::new();
            for i in 0..population {
                let uuid = Uuid::new_v4();
                uuids.push(uuid);
                r.add(at(i as i32, uuid, rng.coord(), rng.coord())).await;
            }
            for uuid in uuids.iter().take(population / 2) {
                r.update_position(uuid, rng.coord(), 64.0, rng.coord(), 0.0, 0.0, true)
                    .await;
            }

            let all = r.get_all().await;
            let snapshot = r.position_snapshot().await;
            for _ in 0..200 {
                let (qx, qz) = (rng.coord(), rng.coord());
                for radius in [0.5, 1.0, 4.0, 16.0, 40.0] {
                    let found: Vec<Uuid> = snapshot
                        .candidates_near(qx, qz, radius)
                        .map(|p| p.uuid)
                        .collect();

                    for p in &all {
                        let (dx, dz) = (p.x - qx, p.z - qz);
                        if (dx * dx + dz * dz).sqrt() <= radius {
                            assert!(
                                found.contains(&p.uuid),
                                "{} players: missed {} at ({}, {}) from ({qx}, {qz}) r={radius}",
                                population,
                                p.username,
                                p.x,
                                p.z
                            );
                        }
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn the_reported_miss() {
        let r = PlayerRegistry::new();
        pad_past_threshold(&r).await;
        let p42 = Uuid::new_v4();
        r.add(at(42, p42, -437.0, -720.0)).await;

        let snapshot = r.position_snapshot().await;
        assert!(snapshot.by_chunk.is_some(), "not bucketed");
        let found: Vec<Uuid> = snapshot
            .candidates_near(-431.0, -718.0, 16.0)
            .map(|p| p.uuid)
            .collect();
        assert!(
            found.contains(&p42),
            "missed p42; found {} others",
            found.len()
        );
    }

    async fn one_player_per_chunk(r: &PlayerRegistry, span: i32) {
        let mut eid = 0;
        for cx in -span..=span {
            for cz in -span..=span {
                r.add(at(
                    eid,
                    Uuid::new_v4(),
                    (cx * 16 + 8) as f64,
                    (cz * 16 + 8) as f64,
                ))
                .await;
                eid += 1;
            }
        }
    }

    fn chunks_visited(snapshot: &PlayerPositions, x: f64, z: f64, r: f64) -> Vec<(i32, i32)> {
        let mut seen: Vec<(i32, i32)> = snapshot
            .candidates_near(x, z, r)
            .map(|p| ((p.x.floor() as i32) >> 4, (p.z.floor() as i32) >> 4))
            .collect();
        seen.sort_unstable();
        seen.dedup();
        seen
    }

    #[tokio::test]
    async fn the_cursor_walks_exactly_the_chunks_in_range() {
        let r = PlayerRegistry::new();
        one_player_per_chunk(&r, 4).await;
        let snapshot = r.position_snapshot().await;
        assert!(snapshot.by_chunk.is_some(), "not bucketed");

        assert_eq!(chunks_visited(&snapshot, 8.0, 8.0, 1.0), vec![(0, 0)]);

        let expected: Vec<(i32, i32)> = (-1..=1)
            .flat_map(|cx| (-1..=1).map(move |cz| (cx, cz)))
            .collect();
        assert_eq!(chunks_visited(&snapshot, 8.0, 8.0, 16.0), expected);

        let expected: Vec<(i32, i32)> = (1..=3)
            .flat_map(|cx| (-4..=-2).map(move |cz| (cx, cz)))
            .collect();
        assert_eq!(chunks_visited(&snapshot, 40.0, -40.0, 16.0), expected);
    }

    #[tokio::test]
    async fn a_lopsided_range_is_not_transposed() {
        let r = PlayerRegistry::new();
        one_player_per_chunk(&r, 4).await;
        let snapshot = r.position_snapshot().await;

        let expected: Vec<(i32, i32)> = (0..=1)
            .flat_map(|cx| (2..=3).map(move |cz| (cx, cz)))
            .collect();
        assert_eq!(
            chunks_visited(&snapshot, 8.0, 40.0, 8.0),
            expected,
            "the x and z ranges were swapped"
        );
    }

    #[tokio::test]
    async fn every_chunk_in_range_is_visited_exactly_once() {
        let r = PlayerRegistry::new();
        one_player_per_chunk(&r, 4).await;
        let snapshot = r.position_snapshot().await;

        let all: Vec<(i32, i32)> = snapshot
            .candidates_near(8.0, 8.0, 40.0)
            .map(|p| ((p.x.floor() as i32) >> 4, (p.z.floor() as i32) >> 4))
            .collect();
        let mut unique = all.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(all.len(), unique.len(), "a chunk was walked twice");
        // x and z each span -32..48, which is chunks -2..=3
        assert_eq!(all.len(), 36);
    }
}
