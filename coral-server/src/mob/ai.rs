use std::sync::Arc;

use coral_types::{MobAttackBroadcast, MobMoveBroadcast};
use coral_world::world::World;
use uuid::Uuid;

use crate::{
    mob::{Mob, MobRegistry, MobType},
    player::registry::{NearbyPlayer, PlayerRegistry},
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
    world: Arc<World>,
) -> (Vec<MobMoveBroadcast>, Vec<MobAttackBroadcast>) {
    let positions = player_registry.position_snapshot().await;

    let mut moves = Vec::new();
    let mut attacks = Vec::new();

    let mut mobs = mob_registry.mobs.write().await;

    for mob in mobs.values_mut() {
        mob.ticks_alive += 1;
        if mob.attack_cooldown > 0 {
            mob.attack_cooldown -= 1;
        }

        let players: Vec<NearbyPlayer> = positions
            .candidates_near(mob.x, mob.z, mob.mob_type.aggro_range() * 1.5)
            .copied()
            .collect();
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
            world
                .get(new_x.floor() as i32, below_y as u8, new_z.floor() as i32)
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

        let height = mob.mob_type.size().height;
        if can_stand_at(&world, new_x, mob.y, mob.z, height).await {
            mob.x = new_x;
        } else if mob.on_ground && can_stand_at(&world, new_x, mob.y + 1.0, mob.z, height).await {
            mob.x = new_x;
            mob.y += 1.0;
        } else {
            mob.vx = 0.0;
        }
        if can_stand_at(&world, mob.x, mob.y, new_z, height).await {
            mob.z = new_z;
        } else if mob.on_ground && can_stand_at(&world, mob.x, mob.y + 1.0, new_z, height).await {
            mob.z = new_z;
            mob.y += 1.0;
        } else {
            mob.vz = 0.0;
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

async fn can_stand_at(world: &Arc<World>, x: f64, y: f64, z: f64, height: f64) -> bool {
    let (bx, bz) = (x.floor() as i32, z.floor() as i32);
    let top = (y + height - 0.001).floor() as i32;
    for by in (y.floor() as i32)..=top {
        if !(0..=255).contains(&by) {
            return false;
        }
        if !world.get(bx, by as u8, bz).await.is_air() {
            return false;
        }
    }
    true
}

fn hostile_targeting(mob: &mut Mob, players: &[NearbyPlayer]) {
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
        let dx = tx - mob.x;
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

#[cfg(test)]
mod movement_tests {
    use std::sync::Arc;

    use coral_world::{blocks::Block, generator::VoidWorldGenerator, world::World};

    use super::{can_stand_at, tick_mob};
    use crate::{
        mob::{Mob, MobRegistry, MobType},
        player::registry::PlayerRegistry,
    };

    const STONE: u8 = 1;

    fn world() -> Arc<World> {
        Arc::new(World::new(Arc::new(VoidWorldGenerator::new())))
    }

    async fn with_floor(world: &Arc<World>, span: i32) {
        for x in -span..=span {
            for z in -span..=span {
                world.set(x, 63, z, Block::new(STONE, 0)).await;
            }
        }
    }

    async fn run(world: Arc<World>, mob: Mob, ticks: usize) -> Mob {
        let mobs = Arc::new(MobRegistry::new());
        let entity_id = mob.entity_id;
        mobs.spawn(mob).await;
        let players = Arc::new(PlayerRegistry::new());
        for _ in 0..ticks {
            tick_mob(mobs.clone(), players.clone(), world.clone()).await;
        }
        mobs.get(entity_id).await.expect("the mob is still there")
    }

    #[tokio::test]
    async fn a_mob_with_velocity_actually_moves() {
        let world = world();
        with_floor(&world, 40).await;

        let mut mob = Mob::new(1, MobType::Cow, 0.5, 64.0, 0.5);
        mob.vx = 0.2;
        mob.vz = 0.1;
        mob.target_pos = Some((30.0, 64.0, 15.0));
        mob.ai_cooldown = 10_000; // keep it from picking a new wander target

        let after = run(world, mob, 100).await;

        assert!(after.x > 0.5, "x never changed (was {})", after.x);
        assert!(after.z > 0.5, "z never changed (was {})", after.z);
    }

    #[tokio::test]
    async fn a_mob_stays_on_the_floor_while_it_walks() {
        let world = world();
        with_floor(&world, 40).await;

        let mut mob = Mob::new(1, MobType::Cow, 0.5, 64.0, 0.5);
        mob.target_pos = Some((20.0, 64.0, 0.5));
        mob.ai_cooldown = 10_000;

        let after = run(world, mob, 100).await;

        assert_eq!(after.y, 64.0, "the mob sank or floated");
        assert!(after.on_ground);
    }

    #[tokio::test]
    async fn a_wall_stops_the_mob_instead_of_letting_it_through() {
        let world = world();
        with_floor(&world, 40).await;
        for y in 64..=65 {
            for z in -40..=40 {
                world.set(3, y, z, Block::new(STONE, 0)).await;
            }
        }

        let mut mob = Mob::new(1, MobType::Cow, 0.5, 64.0, 0.5);
        mob.target_pos = Some((30.0, 64.0, 0.5));
        mob.ai_cooldown = 10_000;

        let after = run(world, mob, 400).await;

        assert!(
            after.x < 3.0,
            "the mob walked into the wall at x=3 ({})",
            after.x
        );
        assert!(
            after.x > 2.0,
            "the mob never reached the wall, so this proves nothing ({})",
            after.x
        );
    }

    #[tokio::test]
    async fn a_single_block_step_is_climbable() {
        let world = world();
        with_floor(&world, 40).await;
        for z in -40..=40 {
            world.set(3, 64, z, Block::new(STONE, 0)).await;
        }

        let mut mob = Mob::new(1, MobType::Cow, 0.5, 64.0, 0.5);
        mob.target_pos = Some((30.0, 64.0, 0.5));
        mob.ai_cooldown = 10_000;

        let after = run(world, mob, 400).await;

        assert!(after.x > 3.0, "the mob could not step up (x = {})", after.x);
    }

    #[tokio::test]
    async fn a_blocked_axis_does_not_block_the_other_one() {
        let world = world();
        with_floor(&world, 40).await;
        for y in 64..=65 {
            for z in -40..=40 {
                world.set(3, y, z, Block::new(STONE, 0)).await;
            }
        }

        let mut mob = Mob::new(1, MobType::Cow, 0.5, 64.0, 0.5);
        mob.target_pos = Some((30.0, 64.0, 20.0));
        mob.ai_cooldown = 10_000;

        let after = run(world, mob, 400).await;

        assert!(after.x < 3.0, "went through the wall");
        assert!(after.x > 2.0, "never reached the wall");
        assert!(after.z > 1.0, "the wall on x also froze z ({})", after.z);
    }

    #[tokio::test]
    async fn a_tall_mob_does_not_fit_through_a_one_block_gap() {
        let world = world();
        with_floor(&world, 10).await;
        for x in -10..=10 {
            for z in -10..=10 {
                world.set(x, 65, z, Block::new(STONE, 0)).await;
            }
        }

        let zombie = MobType::Zombie.size().height;
        assert!(zombie > 1.0);
        assert!(
            !can_stand_at(&world, 0.5, 64.0, 0.5, zombie).await,
            "a 1.95-tall mob was allowed into a one-block gap"
        );
        let pig = MobType::Pig.size().height;
        assert!(
            can_stand_at(&world, 0.5, 64.0, 0.5, pig).await,
            "a 0.9-tall mob should fit"
        );
    }
}
