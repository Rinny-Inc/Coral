use coral_types::ids::blocks;

pub const GRAVITY: f64 = 0.04;
pub const DRAG: f64 = 0.98;
pub const TNT_FUSE_TICKS: u16 = 80;
pub const TNT_POWER: f32 = 4.0;
pub const MAX_FALL_TICKS: u32 = 100;

/// What is falling and what it does when it stops
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallingKind {
    /// Counts down and then detonates
    PrimedTnt { fuse: u16 },
    /// Lands and becomes a block again
    Block { block_id: u8, metadata: u8 },
}

/// One flying entity in flight
#[derive(Debug, Clone)]
pub struct FallingEntity {
    pub entity_id: i32,
    pub kind: FallingKind,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub ticks_alive: u32,
}

pub fn is_gravity_block(block_id: u8) -> bool {
    matches!(
        block_id,
        blocks::SAND | blocks::GRAVEL | blocks::ANVIL | blocks::DRAGON_EGG
    )
}

pub fn can_fall_through(block_id: u8) -> bool {
    matches!(
        block_id,
        blocks::AIR
            | blocks::FIRE
            | blocks::FLOWING_WATER
            | blocks::WATER
            | blocks::FLOWING_LAVA
            | blocks::LAVA
    )
}

pub fn should_start_falling(block_id: u8, below_id: u8, y: i32) -> bool {
    is_gravity_block(block_id) && y > 0 && can_fall_through(below_id)
}

pub fn spawn_object_type(kind: &FallingKind) -> u8 {
    match kind {
        FallingKind::PrimedTnt { .. } => 50,
        FallingKind::Block { .. } => 70,
    }
}

pub fn spawn_object_data(kind: &FallingKind) -> i32 {
    match kind {
        FallingKind::PrimedTnt { .. } => 0,
        FallingKind::Block { block_id, metadata } => *block_id as i32 | ((*metadata as i32) << 12),
    }
}

pub fn tnt_launch_velocity(angle_roll: f64) -> (f64, f64, f64) {
    let angle = angle_roll * std::f64::consts::TAU;
    (
        -(angle.sin()) * 0.02,
        0.200_000_002_980_232_24,
        -(angle.cos()) * 0.02,
    )
}

pub fn chained_fuse(roll: f64) -> u16 {
    let quarter = (TNT_FUSE_TICKS / 4) as f64;
    (TNT_FUSE_TICKS / 8) + (roll * quarter) as u16
}

pub fn step(entity: &FallingEntity) -> ((f64, f64, f64), (f64, f64, f64)) {
    let vy = entity.vy - GRAVITY;
    let (vx, vz) = match entity.kind {
        FallingKind::Block { .. } => (0.0, 0.0),
        FallingKind::PrimedTnt { .. } => (entity.vx, entity.vz),
    };
    let position = (entity.x + vx, entity.y + vy, entity.z + vz);
    let velocity = (vx * DRAG, vy * DRAG, vz * DRAG);
    (position, velocity)
}

pub fn bounce(velocity: (f64, f64, f64)) -> (f64, f64, f64) {
    (velocity.0 * 0.7, velocity.1 * -0.5, velocity.2 * 0.7)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tnt() -> FallingEntity {
        FallingEntity {
            entity_id: 1,
            kind: FallingKind::PrimedTnt {
                fuse: TNT_FUSE_TICKS,
            },
            x: 0.5,
            y: 64.0,
            z: 0.5,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            ticks_alive: 0,
        }
    }

    fn sand() -> FallingEntity {
        FallingEntity {
            entity_id: 2,
            kind: FallingKind::Block {
                block_id: blocks::SAND,
                metadata: 0,
            },
            ..tnt()
        }
    }

    #[test]
    fn only_the_four_gravity_blocks_fall() {
        for id in [
            blocks::SAND,
            blocks::GRAVEL,
            blocks::ANVIL,
            blocks::DRAGON_EGG,
        ] {
            assert!(is_gravity_block(id), "block {id} should fall");
        }
        for id in [blocks::STONE, blocks::DIRT, blocks::TNT, blocks::GLASS] {
            assert!(!is_gravity_block(id), "block {id} should not fall");
        }
    }

    #[test]
    fn sand_falls_through_air_and_liquid_but_rests_on_plants() {
        assert!(can_fall_through(blocks::AIR));
        assert!(can_fall_through(blocks::WATER));
        assert!(can_fall_through(blocks::FLOWING_WATER));
        assert!(can_fall_through(blocks::LAVA));
        assert!(can_fall_through(blocks::FIRE));
        assert!(!can_fall_through(blocks::TALLGRASS));
        assert!(!can_fall_through(blocks::TORCH));
        assert!(!can_fall_through(blocks::STONE));
    }

    #[test]
    fn a_block_only_starts_falling_with_nothing_under_it() {
        assert!(should_start_falling(blocks::SAND, blocks::AIR, 64));
        assert!(!should_start_falling(blocks::SAND, blocks::STONE, 64));
        assert!(!should_start_falling(blocks::STONE, blocks::AIR, 64));
        assert!(!should_start_falling(blocks::SAND, blocks::AIR, 0));
    }

    #[test]
    fn gravity_builds_up_over_a_fall() {
        let mut entity = sand();
        let mut speeds = Vec::new();
        for _ in 0..5 {
            let (position, velocity) = step(&entity);
            speeds.push(velocity.1);
            (entity.x, entity.y, entity.z) = position;
            (entity.vx, entity.vy, entity.vz) = velocity;
        }
        for pair in speeds.windows(2) {
            assert!(pair[1] < pair[0], "a fall should accelerate: {speeds:?}");
        }
        assert!(entity.y < 64.0, "it should have moved down");
    }

    #[test]
    fn the_first_tick_already_carries_speed() {
        let (position, _) = step(&sand());
        assert!(position.1 < 64.0);
        assert_eq!(position.1, 64.0 - GRAVITY);
    }

    #[test]
    fn a_falling_block_does_not_drift_sideways() {
        let mut entity = sand();
        entity.vx = 5.0;
        entity.vz = -5.0;
        let (position, velocity) = step(&entity);
        assert_eq!(position.0, 0.5);
        assert_eq!(position.2, 0.5);
        assert_eq!((velocity.0, velocity.2), (0.0, 0.0));
    }

    #[test]
    fn primed_tnt_keeps_the_nudge_it_was_lit_with() {
        let mut entity = tnt();
        let (vx, vy, vz) = tnt_launch_velocity(0.125);
        entity.vx = vx;
        entity.vy = vy;
        entity.vz = vz;
        let (position, _) = step(&entity);
        assert!(position.0 != 0.5 || position.2 != 0.5, "TNT should scatter");
        assert!(position.1 > 64.0);
    }

    #[test]
    fn the_launch_nudge_is_small_and_points_somewhere() {
        for roll in [0.0, 0.25, 0.5, 0.75, 0.99] {
            let (vx, vy, vz) = tnt_launch_velocity(roll);
            assert!((vy - 0.2).abs() < 1e-7, "the hop is fixed");
            let horizontal = (vx * vx + vz * vz).sqrt();
            assert!(
                (horizontal - 0.02).abs() < 1e-9,
                "roll {roll} gave a nudge of {horizontal}"
            );
        }
        assert_ne!(tnt_launch_velocity(0.0), tnt_launch_velocity(0.25));
    }

    #[test]
    fn a_chained_fuse_is_short_and_staggered() {
        let fuses: Vec<u16> = [0.0, 0.25, 0.5, 0.75, 0.999]
            .iter()
            .map(|&r| chained_fuse(r))
            .collect();
        for &fuse in &fuses {
            assert!(fuse >= 10, "fuse {fuse} is too short to scatter");
            assert!(
                fuse < TNT_FUSE_TICKS,
                "a chained fuse should beat a lit one"
            );
        }
        assert!(
            fuses.first() != fuses.last(),
            "the roll should stagger them: {fuses:?}"
        );
    }

    #[test]
    fn a_bounce_damps_and_reverses() {
        let (vx, vy, vz) = bounce((1.0, -0.5, -1.0));
        assert!(vy > 0.0, "it should come back up");
        assert!(vy.abs() < 0.5, "but not as fast as it came down");
        assert!(
            vx.abs() < 1.0 && vz.abs() < 1.0,
            "and lose horizontal speed"
        );
    }

    #[test]
    fn the_spawn_packet_describes_each_kind() {
        let tnt_kind = FallingKind::PrimedTnt { fuse: 80 };
        assert_eq!(spawn_object_type(&tnt_kind), 50);
        assert_eq!(spawn_object_data(&tnt_kind), 0);

        let sand_kind = FallingKind::Block {
            block_id: blocks::SAND,
            metadata: 0,
        };
        assert_eq!(spawn_object_type(&sand_kind), 70);
        assert_eq!(spawn_object_data(&sand_kind), blocks::SAND as i32);
    }

    #[test]
    fn a_falling_block_carries_its_metadata() {
        let red_sand = FallingKind::Block {
            block_id: blocks::SAND,
            metadata: 1,
        };
        let data = spawn_object_data(&red_sand);
        assert_eq!(data & 0xFFF, blocks::SAND as i32);
        assert_eq!(data >> 12, 1);
        assert_ne!(data, 0);
    }
}
