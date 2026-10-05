use std::sync::Arc;

use coral_types::{MobAttackBroadcast, MobMoveBroadcast};
use coral_world::{blocks::WorldBlocks, generator::FlatWorldGenerator};
use uuid::Uuid;

use crate::{
    mob::{Mob, MobRegistry, MobType},
    player::{Player, registry::PlayerRegistry},
};

const WANDER_MIN_COOLDOWN: u32 = 100;
const WANDER_COOLDOWN_JITTER: u32 = 100;
const WANDER_MIN_DIST: f64 = 4.0;
const WANDER_EXTRA_DIST: f64 = 6.0;
const ARRIVAL_TRESHOLD: f64 = 0.5;
const ATTACK_RANGE: f64 = 2.0;
const ATTACK_COOLDOWN_TICKS: u32 = 20;
const GRAVITY: f64 = 0.08;
const DRAG: f64 = 0.98;
const IDLE_FRICTION: f64 = 0.6;

// FIXME: not implemented
pub async fn tick_mob(
    mob_registry: Arc<MobRegistry>,
    player_registry: Arc<PlayerRegistry>,
    world_blocks: Arc<WorldBlocks>,
    generator: Arc<FlatWorldGenerator>,
) -> (Vec<MobMoveBroadcast>, Vec<MobAttackBroadcast>) {
    let players = player_registry.get_all().await;

    let mut moves = Vec::new();
    let mut attacks = Vec::new();

    let mut mobs = mob_registry.mobs.write().await;

    for mob in mobs.values_mut() {
        mob.ticks_alive += 1;
        if mob.attack_cooldown > 0 {
            mob.attack_cooldown -= 1;
        }

        hostile_targeting(mob, &players);

        let chase_target = mob
            .target_player
            .and_then(|uuid| players.iter().find(|p| p.uuid == uuid))
            .map(|p| (p.x, p.y, p.z, p.uuid));

        if let Some((tx, ty, tz, target_uuid)) = chase_target {
            chase_player(mob, &target_uuid, tx, ty, tz, &mut attacks);
        } else {
            wander(mob);
        }

        if !mob.on_ground {
            mob.vy -= GRAVITY;
            mob.vy *= DRAG;
        }

        let new_x = mob.x + mob.vx;
        let new_y = mob.y + mob.vy;
        let new_z = mob.z + mob.vz;

        let below_y = (new_y - 0.1).floor();
        let below = if (0.0..=255.0).contains(&below_y) {
            world_blocks
                .get(
                    new_x.floor() as i32,
                    below_y as u8,
                    new_z.floor() as i32,
                    &generator,
                )
                .await
        } else {
            coral_world::blocks::Block::air()
        };

        if !below.is_air() && mob.vy <= 0.0 {
            mob.vy = 0.0;
            mob.on_ground = true;
            mob.y = below_y + 1.0;
        } else {
            mob.on_ground = false;
            mob.y = new_y;
        }

        moves.push((
            mob.entity_id,
            mob.x,
            mob.y,
            mob.z,
            mob.yaw,
            mob.pitch,
            mob.head_yaw,
            mob.on_ground,
        ));
    }

    (moves, attacks)
}

fn hostile_targeting(mob: &mut Mob, players: &Vec<Player>) {
    if !mob.mob_type.is_hostile() {
        return;
    }
    let range = mob.mob_type.aggro_range();

    if let Some(target_uuid) = mob.target_player {
        let still_valid = players.iter().any(|p| {
            p.uuid == target_uuid && !p.is_dead && mob.distance_to(p.x, p.y, p.z) <= range * 1.5
        });
        if !still_valid {
            mob.target_player = None;
        }
    }

    if mob.target_player.is_none() {
        let nearest = players
            .iter()
            .filter(|p| !p.is_dead)
            .map(|p| (p, mob.distance_to(p.x, p.y, p.z)))
            .filter(|(_, d)| *d <= range)
            .min_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((p, _)) = nearest {
            mob.target_player = Some(p.uuid);
            mob.target_pos = None; // chasing > wandering
        }
    }
}

fn chase_player(
    mob: &mut Mob,
    target_uuid: &Uuid,
    tx: f64,
    ty: f64,
    tz: f64,
    attacks: &mut Vec<(Uuid, f32, i32)>,
) {
    let dist = mob.distance_to(tx, ty, tz);

    if dist <= ATTACK_RANGE {
        mob.vx = 0.0;
        mob.vz = 0.0;
        let damage = mob.mob_type.attack_damage();
        if damage > 0.0 && mob.attack_cooldown == 0 {
            attacks.push((*target_uuid, damage, mob.entity_id));
            mob.attack_cooldown = ATTACK_COOLDOWN_TICKS;
        }
    } else {
        let dx = tz - mob.x;
        let dz = tz - mob.z;
        let horizontal = (dx * dx + dz * dz).sqrt().max(0.0001);
        let speed = mob.mob_type.movement_speed() * 0.1;
        mob.vx = (dx / horizontal) * speed;
        mob.vz = (dz / horizontal) * speed;
    }

    // always face the target
    let dx = tx - mob.x;
    let dz = tz - mob.z;
    mob.yaw = (dz.atan2(dx).to_degrees() as f32) - 90.0;
    mob.head_yaw = mob.yaw;
    mob.pitch = {
        let horizontal = (dx * dx + dz * dz).sqrt();
        let dy = (ty + 1.62) - (mob.y + mob.mob_type.size().height);
        (-dy).atan2(horizontal).to_degrees() as f32
    };
}

fn wander(mob: &mut Mob) {
    if mob.ai_cooldown == 0 {
        let angle = rand::random::<f64>() * std::f64::consts::TAU;
        let dist = WANDER_MIN_DIST + rand::random::<f64>() * WANDER_EXTRA_DIST;
        mob.target_pos = Some((
            mob.x + angle.cos() * dist,
            mob.y,
            mob.z + angle.sin() * dist,
        ));
        mob.ai_cooldown = WANDER_MIN_COOLDOWN + (rand::random::<u32>() % WANDER_COOLDOWN_JITTER);
    } else {
        mob.ai_cooldown -= 1;
    }

    if let Some((tx, _, tz)) = mob.target_pos {
        let dx = tx - mob.x;
        let dz = tz - mob.z;
        let dist = (dx * dx + dz * dz).sqrt();

        if dist < ARRIVAL_TRESHOLD {
            mob.target_pos = None;
            mob.vx = 0.0;
            mob.vz = 0.0;
        } else {
            let speed = mob.mob_type.movement_speed() * 0.1;
            mob.vx = (dx / dist) * speed;
            mob.vz = (dz / dist) * speed;
            mob.yaw = (dz.atan2(dx).to_degrees() as f32) - 90.0;
            mob.head_yaw = mob.yaw;
            mob.pitch = 0.0;
        }
        return;
    }
    mob.vx *= IDLE_FRICTION;
    mob.vz *= IDLE_FRICTION;
}

pub fn roll_drops(mob_type: MobType) -> Vec<(i16, u8)> {
    mob_type
        .drops()
        .iter()
        .filter_map(|&(item_id, min, max)| {
            let count = if max > min {
                min + (rand::random::<u32>() % ((max - min + 1) as u32)) as u8
            } else {
                min
            };
            if count == 0 {
                None
            } else {
                Some((item_id, count))
            }
        })
        .collect()
}
