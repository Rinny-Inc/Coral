use std::collections::HashMap;

use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EntityType {
    Player,
    Mob,
    Item,
    ExperienceOrb,
}

pub struct TrackedEntity {
    pub entity_id: i32,
    pub uuid: Option<Uuid>,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub entity_type: EntityType,
    pub tracking_range: f64,
}

impl TrackedEntity {
    pub fn player(entity_id: i32, uuid: Uuid, x: f64, y: f64, z: f64, range: f64) -> Self {
        Self {
            entity_id,
            uuid: Some(uuid),
            x,
            y,
            z,
            entity_type: EntityType::Player,
            tracking_range: range,
        }
    }

    pub fn item(entity_id: i32, x: f64, y: f64, z: f64, range: f64) -> Self {
        Self {
            entity_id,
            uuid: None,
            x,
            y,
            z,
            entity_type: EntityType::Item,
            tracking_range: range,
        }
    }

    pub fn mob(entity_id: i32, x: f64, y: f64, z: f64, range: f64) -> Self {
        Self {
            entity_id,
            uuid: None,
            x,
            y,
            z,
            entity_type: EntityType::Mob,
            tracking_range: range,
        }
    }

    pub fn experience_orb(entity_id: i32, x: f64, y: f64, z: f64, range: f64) -> Self {
        Self {
            entity_id,
            uuid: None,
            x,
            y,
            z,
            entity_type: EntityType::ExperienceOrb,
            tracking_range: range,
        }
    }
}

pub struct EntityTracker {
    pub entities: HashMap<i32, TrackedEntity>,
}
impl EntityTracker {
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
        }
    }

    pub fn track(&mut self, entity: TrackedEntity) {
        self.entities.insert(entity.entity_id, entity);
    }

    pub fn untrack(&mut self, entity_id: i32) {
        self.entities.remove(&entity_id);
    }

    pub fn update_position(&mut self, entity_id: i32, x: f64, y: f64, z: f64) {
        if let Some(e) = self.entities.get_mut(&entity_id) {
            e.x = x;
            e.y = y;
            e.z = z;
        }
    }

    pub fn is_visible_to(&self, entity_id: i32, viewer_x: f64, viewer_z: f64) -> bool {
        if let Some(e) = self.entities.get(&entity_id) {
            let dx = e.x - viewer_x;
            let dz = e.z - viewer_z;
            let dist_sq = dx * dx + dz * dz;
            dist_sq <= e.tracking_range * e.tracking_range
        } else {
            false
        }
    }

    pub fn get_visible_for(&self, viewer_x: f64, viewer_z: f64) -> Vec<&TrackedEntity> {
        self.entities
            .values()
            .filter(|e| {
                let dx = e.x - viewer_x;
                let dz = e.z - viewer_z;
                let dist_sq = dx * dx + dz * dz;
                dist_sq <= e.tracking_range * e.tracking_range
            })
            .collect()
    }
}
