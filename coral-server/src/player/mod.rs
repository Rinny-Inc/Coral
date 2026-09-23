use std::sync::Arc;

use coral_types::GameMode;
use coral_world::{
    blocks::{WorldBlocks, fluid::Fluid},
    generator::FlatWorldGenerator,
};
use uuid::Uuid;

use coral_protocol::auth::ProfileProperty;

use crate::{bounding_box::EntityBounds, effects::ActiveEffect};

pub mod registry;

#[derive(Debug, Clone)]
pub struct Player {
    pub entity_id: i32,
    pub uuid: Uuid,
    pub username: Arc<String>,
    pub properties: Arc<[ProfileProperty]>,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub velocity: (f64, f64, f64),
    pub gamemode: GameMode,
    pub held_slot: u8,
    pub held_item_id: i16,
    pub latency_ms: i32,
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
    pub is_sleeping: bool,
    pub air_tick: i16,
    pub on_fire: bool,
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
            username: username.into(),
            properties: properties.into(),
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
            is_sleeping: false,
            velocity: (0.0, 0.0, 0.0),
            air_tick: 300,
            on_fire: false,
        }
    }

    pub fn get_head_direction(&self) -> (f64, f64, f64) {
        let yaw_rad = self.yaw * std::f32::consts::PI / 180.0;
        let pitch_rad = self.pitch * std::f32::consts::PI / 180.0;
        let dx = (-yaw_rad.sin() * pitch_rad.cos()) as f64;
        let dy = (-pitch_rad.sin()) as f64;
        let dz = (yaw_rad.cos() * pitch_rad.cos()) as f64;
        (dx, dy, dz)
    }

    pub fn get_head_position(&self) -> (f64, f64, f64) {
        let eye_height = EntityBounds::player(self.is_sneaking).height;
        (self.x, self.y + eye_height, self.z)
    }

    pub async fn is_head_submerged(
        &self,
        wb: &Arc<WorldBlocks>,
        generator: &Arc<FlatWorldGenerator>,
    ) -> bool {
        let (bx, by, bz) = self.get_head_position();
        let by = by.floor() as i32;

        if !(0..=255).contains(&by) {
            return false;
        }
        let bx = bx.floor() as i32;
        let bz = bz.floor() as i32;

        let block = wb.get(bx, by as u8, bz, generator);
        Fluid::is_water(block.await.id)
    }

    pub fn entity_flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.on_fire {
            flags |= 0x01;
        }
        if self.is_sneaking {
            flags |= 0x02;
        }
        if self.is_sprinting {
            flags |= 0x08;
        }
        flags
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::registry::PlayerRegistry;
    use coral_protocol::auth::ProfileProperty;

    fn player(name: &str) -> Player {
        Player::new(
            1,
            Uuid::new_v4(),
            name.to_string(),
            vec![ProfileProperty {
                name: "textures".to_string(),
                value: "x".repeat(400),
                signature: Some("s".repeat(684)),
            }],
            0.5,
            64.0,
            0.5,
            0.0,
            0.0,
            GameMode::Survival,
            20.0,
            20,
            5.0,
        )
    }

    #[test]
    fn clone_shares_the_immutable_fields() {
        let a = player("Noksio");
        let b = a.clone();
        assert!(
            Arc::ptr_eq(&a.username, &b.username),
            "cloning a Player must not copy the username"
        );
        assert!(
            Arc::ptr_eq(&a.properties, &b.properties),
            "cloning a Player must not copy the skin properties"
        );
        assert_eq!(&*b.username, "Noksio");
        assert_eq!(b.properties.len(), 1);
    }

    #[test]
    fn clones_are_still_independent_for_mutable_state() {
        let a = player("Noksio");
        let mut b = a.clone();
        b.health = 3.0;
        b.x = 100.0;
        assert_eq!(a.health, 20.0);
        assert_eq!(a.x, 0.5);
    }

    #[tokio::test]
    async fn registry_reads_share_with_the_stored_player() {
        let registry = PlayerRegistry::new();
        let p = player("Noksio");
        let uuid = p.uuid;
        let username = p.username.clone();
        registry.add(p).await;

        let got = registry.get(&uuid).await.expect("player is registered");
        assert!(
            Arc::ptr_eq(&got.username, &username),
            "registry.get must hand back a shared username, not a copy"
        );

        let all = registry.get_all().await;
        assert_eq!(all.len(), 1);
        assert!(Arc::ptr_eq(&all[0].properties, &got.properties));
    }
}
