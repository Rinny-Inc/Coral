use std::collections::HashMap;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::bounding_box::{BoundingBox, EntityBounds};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MobType {
    // Passive
    Cow,
    Pig,
    Sheep,
    Chicken,
    // Hostile
    Zombie,
    Skeleton,
    Spider,
    Creeper,
}
impl MobType {
    pub const ALL_NAMES: &'static str =
        "cow, pig, sheep, chicken, zombie, skeleton, spider, creeper";

    pub fn entity_id(&self) -> u8 {
        match self {
            MobType::Creeper => 50,
            MobType::Skeleton => 51,
            MobType::Spider => 52,
            MobType::Zombie => 54,
            MobType::Pig => 90,
            MobType::Sheep => 91,
            MobType::Cow => 92,
            MobType::Chicken => 93,
        }
    }

    pub fn max_health(&self) -> f32 {
        match self {
            MobType::Cow | MobType::Pig | MobType::Sheep => 10.0,
            MobType::Chicken => 4.0,
            MobType::Zombie | MobType::Skeleton | MobType::Creeper => 20.0,
            MobType::Spider => 16.0,
        }
    }

    pub fn is_hostile(&self) -> bool {
        matches!(
            self,
            MobType::Zombie | MobType::Skeleton | MobType::Spider | MobType::Creeper
        )
    }

    pub fn size(&self) -> BoundingBox {
        match self {
            MobType::Cow => EntityBounds::cow(),
            MobType::Sheep => EntityBounds::sheep(),
            MobType::Pig => EntityBounds::pig(),
            MobType::Chicken => EntityBounds::chicken(),
            MobType::Zombie => EntityBounds::zombie(),
            MobType::Skeleton => EntityBounds::skeleton(),
            MobType::Creeper => EntityBounds::creeper(),
            MobType::Spider => EntityBounds::spider(),
        }
    }

    pub fn movement_speed(&self) -> f64 {
        match self {
            MobType::Cow | MobType::Sheep => 0.20,
            MobType::Pig | MobType::Chicken => 0.25,
            MobType::Zombie => 0.23,
            MobType::Skeleton | MobType::Creeper => 0.25,
            MobType::Spider => 0.30,
        }
    }

    pub fn attack_damage(&self) -> f32 {
        match self {
            MobType::Zombie => 3.0,
            MobType::Spider => 2.0,
            MobType::Skeleton => 0.0, // ranged - damage by arrow
            MobType::Creeper => 0.0,  // damage by explosion
            _ => 0.0,
        }
    }

    pub fn aggro_range(&self) -> f64 {
        if self.is_hostile() { 16.0 } else { 0.0 }
    }

    pub fn drops(&self) -> &[(i16, u8, u8)] {
        match self {
            MobType::Cow => &[(334, 0, 2), (363, 1, 3)], // leather, raw beef
            MobType::Pig => &[(319, 1, 3)],              // raw porkchop
            MobType::Sheep => &[(35, 1, 1)],             // wool
            MobType::Chicken => &[(288, 0, 2), (365, 1, 1)], // feather, raw chicken
            MobType::Zombie => &[(367, 0, 2)],           // rotten flesh
            MobType::Skeleton => &[(352, 0, 2), (262, 0, 2)], // bone, arrow
            MobType::Spider => &[(287, 0, 2), (375, 0, 1)], // string, spider eye
            MobType::Creeper => &[(289, 0, 2)],          // gunpowder
        }
    }

    pub fn xp_on_death(&self) -> i32 {
        if self.is_hostile() {
            5
        } else {
            1 + (rand::random::<i32>() % 3) as i32
        }
    }

    pub fn from_name(name: &str) -> Option<MobType> {
        Some(match name.to_lowercase().as_str() {
            "cow" => MobType::Cow,
            "pig" => MobType::Pig,
            "sheep" => MobType::Sheep,
            "chicken" => MobType::Chicken,
            "zombie" => MobType::Zombie,
            "skeleton" => MobType::Skeleton,
            "spider" => MobType::Spider,
            "creeper" => MobType::Creeper,
            _ => return None,
        })
    }

    pub fn name(&self) -> &'static str {
        match self {
            MobType::Cow => "cow",
            MobType::Pig => "pig",
            MobType::Sheep => "sheep",
            MobType::Chicken => "chicken",
            MobType::Zombie => "zombie",
            MobType::Skeleton => "skeleton",
            MobType::Spider => "spider",
            MobType::Creeper => "creeper",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Mob {
    pub entity_id: i32,
    pub uuid: Uuid,
    pub mob_type: MobType,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub head_yaw: f32,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub health: f32,
    pub on_ground: bool,
    pub target_pos: Option<(f64, f64, f64)>,
    pub target_player: Option<Uuid>,
    pub ai_cooldown: u32,
    pub attack_cooldown: u32,
    pub ticks_alive: u32,
}
impl Mob {
    pub fn new(entity_id: i32, mob_type: MobType, x: f64, y: f64, z: f64) -> Self {
        Self {
            entity_id,
            uuid: Uuid::new_v4(),
            mob_type: mob_type.clone(),
            x,
            y,
            z,
            yaw: 0.0,
            pitch: 0.0,
            head_yaw: 0.0,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            health: mob_type.max_health(),
            on_ground: false,
            target_pos: None,
            target_player: None,
            ai_cooldown: 0,
            attack_cooldown: 0,
            ticks_alive: 0,
        }
    }

    pub fn is_dead(&self) -> bool {
        self.health <= 0.0
    }

    pub fn distance_to(&self, x: f64, y: f64, z: f64) -> f64 {
        let dx = self.x - x;
        let dy = self.y - y;
        let dz = self.z - z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

pub struct MobRegistry {
    pub mobs: RwLock<HashMap<i32, Mob>>,
}
impl MobRegistry {
    pub fn new() -> Self {
        Self {
            mobs: RwLock::new(HashMap::new()),
        }
    }

    pub async fn spawn(&self, mob: Mob) {
        self.mobs.write().await.insert(mob.entity_id, mob);
    }

    pub async fn remove(&self, entity_id: i32) -> Option<Mob> {
        self.mobs.write().await.remove(&entity_id)
    }

    pub async fn get(&self, entity_id: i32) -> Option<Mob> {
        self.mobs.read().await.get(&entity_id).cloned()
    }

    pub async fn all(&self) -> Vec<Mob> {
        self.mobs.read().await.values().cloned().collect()
    }

    pub async fn count(&self) -> usize {
        self.mobs.read().await.len()
    }

    pub async fn damage(&self, entity_id: i32, amount: f32) -> Option<(f32, bool)> {
        let mut mobs = self.mobs.write().await;
        let mob = mobs.get_mut(&entity_id)?;
        mob.health = (mob.health - amount).max(0.0);
        Some((mob.health, mob.health <= 0.0))
    }
}
impl Default for MobRegistry {
    fn default() -> Self {
        Self::new()
    }
}
