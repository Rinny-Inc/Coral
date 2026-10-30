use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use coral_types::ext::TicksExt;
use coral_world::blocks::Block;
use coral_world::world::World;
use tokio::sync::RwLock;
use tokio::time::interval;

use coral_world::blocks::fluid::{Fluid, FluidKind, is_replaceable};

use crate::Channels;

const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

#[derive(Default)]
pub struct FluidQueue {
    order: VecDeque<(i32, i32, i32)>,
    queued: HashSet<(i32, i32, i32)>,
}
impl FluidQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, pos: (i32, i32, i32)) -> bool {
        if self.queued.insert(pos) {
            self.order.push_back(pos);
            true
        } else {
            false
        }
    }

    pub fn drain_batch(&mut self, max: usize) -> Vec<(i32, i32, i32)> {
        let n = self.order.len().min(max);
        let batch: Vec<(i32, i32, i32)> = self.order.drain(..n).collect();
        for pos in &batch {
            self.queued.remove(pos);
        }
        batch
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }
}

pub async fn queue_fluid_update(x: i32, y: i32, z: i32, fluid_queue: &Arc<RwLock<FluidQueue>>) {
    let mut q = fluid_queue.write().await;
    for (dx, dy, dz) in [
        (0, 0, 0),
        (0, 1, 0),
        (0, -1, 0),
        (1, 0, 0),
        (-1, 0, 0),
        (0, 0, 1),
        (0, 0, -1),
    ] {
        let pos = (x + dx, y + dy, z + dz);
        if (0..=255).contains(&pos.1) {
            q.push(pos);
        }
    }
}

pub fn spawn_fluid_task(
    fluid_queue: Arc<RwLock<FluidQueue>>,
    world: Arc<World>,
    channels: Channels,
) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_ticks(5));
        let mut tick: u64 = 0;
        let mut shutdown_rx = channels.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    tick += 5;

                    let batch: Vec<(i32, i32, i32)> = {
                        fluid_queue.write().await.drain_batch(200)
                    };

                    for (x, y, z) in batch {
                        if !(0..=255).contains(&y) {
                            continue;
                        }
                        let block = world.get(x, y as u8, z).await;
                        let Some(fluid) = Fluid::from_block_id(block.id) else {
                            continue;
                        };

                        let rate = match fluid.kind {
                            FluidKind::Water => 5,
                            FluidKind::Lava => 30,
                        };
                        if !tick.is_multiple_of(rate) {
                            fluid_queue.write().await.push((x, y, z));
                            continue;
                        }

                        update_tick(
                            x,
                            y,
                            z,
                            fluid,
                            block.metadata,
                            &world,
                            &channels,
                            &fluid_queue,
                        )
                        .await;
                    }
                }
                Ok(_) = shutdown_rx.recv() => {
                    tracing::info!("[Fluid] shutting down");
                    break;
                }
            }
        }
    });
}

async fn decay_at(fluid: Fluid, x: i32, y: i32, z: i32, world: &Arc<World>) -> i32 {
    if !(0..=255).contains(&y) {
        return -1;
    }
    let b = world.get(x, y as u8, z).await;
    if Fluid::same_kind(b.id, fluid.block_id()) {
        b.metadata as i32
    } else {
        -1
    }
}

fn fold_min_decay(neighbor_decay: i32, current: i32, sources: &mut u8) -> i32 {
    let mut d = neighbor_decay;
    if d < 0 {
        return current;
    }
    if d == 0 {
        *sources += 1;
    }
    if d >= 8 {
        d = 0;
    }
    if current >= 0 && d >= current {
        current
    } else {
        d
    }
}

async fn is_blocked(x: i32, y: i32, z: i32, world: &Arc<World>) -> bool {
    if !(0..=255).contains(&y) {
        return true;
    }
    let b = world.get(x, y as u8, z).await;
    !(b.is_air() || is_replaceable(b.id) || Fluid::is_fluid(b.id))
}

async fn can_flow_into(fluid: Fluid, x: i32, y: i32, z: i32, world: &Arc<World>) -> bool {
    if !(0..=255).contains(&y) {
        return false;
    }
    let b = world.get(x, y as u8, z).await;
    if Fluid::same_kind(b.id, fluid.block_id()) {
        return false;
    }
    if Fluid::is_lava(b.id) {
        return false;
    }
    !is_blocked(x, y, z, world).await
}

async fn flow_into(
    x: i32,
    y: i32,
    z: i32,
    fluid: Fluid,
    decay: u8,
    world: &Arc<World>,
    channels: &Channels,
    queue: &Arc<RwLock<FluidQueue>>,
) {
    if !can_flow_into(fluid, x, y, z, world).await {
        return;
    }
    if fluid.kind == FluidKind::Lava {
        fizz(x, y, z, channels);
    }
    let flowing = fluid.flowing_variant();
    set_block(x, y, z, flowing.block_id(), decay, world, channels, queue).await;
}

fn decay_step(fluid: Fluid) -> i32 {
    match fluid.kind {
        FluidKind::Water => 1,
        FluidKind::Lava => 2,
    }
}

async fn buildable_or_source_below(
    x: i32,
    y: i32,
    z: i32,
    fluid: Fluid,
    world: &Arc<World>,
) -> bool {
    if y == 0 {
        return true;
    }
    let below = world.get(x, (y - 1) as u8, z).await;
    let solid = !below.is_air() && !is_replaceable(below.id) && !Fluid::is_fluid(below.id);
    let same_source =
        Fluid::same_kind(below.id, fluid.block_id()) && Fluid::is_source(below.id, below.metadata);
    solid || same_source
}

async fn check_lava_water_reaction(
    x: i32,
    y: i32,
    z: i32,
    fluid: Fluid,
    metadata: u8,
    world: &Arc<World>,
) -> Option<u8> {
    if fluid.kind != FluidKind::Lava {
        return None;
    }
    let mut water_adjacent = false;
    for (dx, dz) in DIRS {
        let n = world.get(x + dx, y as u8, z + dz).await;
        if Fluid::is_water(n.id) {
            water_adjacent = true;
            break;
        }
    }
    if !water_adjacent && y < 255 {
        let above = world.get(x, (y + 1) as u8, z).await;
        if Fluid::is_water(above.id) {
            water_adjacent = true;
        }
    }
    if !water_adjacent {
        return None;
    }
    let is_source = Fluid::is_source(fluid.block_id(), metadata);
    Some(if is_source { 49 } else { 4 }) // obsidian : cobblestone
}

async fn update_tick(
    x: i32,
    y: i32,
    z: i32,
    fluid: Fluid,
    metadata: u8,
    world: &Arc<World>,
    channels: &Channels,
    queue: &Arc<RwLock<FluidQueue>>,
) {
    if let Some(converted) = check_lava_water_reaction(x, y, z, fluid, metadata, world).await {
        set_block(x, y, z, converted, 0, world, channels, queue).await;
        fizz(x, y, z, channels);
        return;
    }

    let step = decay_step(fluid);
    let mut l: i32 = metadata as i32;

    if l > 0 {
        let mut sources: u8 = 0;
        let mut k1: i32 = -100;
        for (dx, dz) in DIRS {
            let nd = decay_at(fluid, x + dx, y, z + dz, world).await;
            k1 = fold_min_decay(nd, k1, &mut sources);
        }

        let mut j1 = k1 + step;
        if j1 >= 8 || k1 < 0 {
            j1 = -1;
        }

        let above = decay_at(fluid, x, y + 1, z, world).await;
        if above >= 0 {
            j1 = if above >= 8 { above } else { above + 8 };
        }

        if sources >= 2
            && fluid.kind == FluidKind::Water
            && buildable_or_source_below(x, y, z, fluid, world).await
        {
            j1 = 0;
        }

        if j1 != l {
            l = j1;
            if j1 < 0 {
                set_block(x, y, z, 0, 0, world, channels, queue).await; // the drain
                return;
            } else {
                set_block(
                    x,
                    y,
                    z,
                    fluid.flowing_variant().block_id(),
                    j1 as u8,
                    world,
                    channels,
                    queue,
                )
                .await;
            }
        }
    }
    if y > 0 && can_flow_into(fluid, x, y - 1, z, world).await {
        if fluid.kind == FluidKind::Lava {
            let below = world.get(x, (y - 1) as u8, z).await;
            if Fluid::is_water(below.id) {
                set_block(x, y - 1, z, 1 /* stone */, 0, world, channels, queue).await;
                fizz(x, y - 1, z, channels);
                return;
            }
        }
        let new_decay = if l >= 8 { l as u8 } else { (l + 8) as u8 };
        flow_into(x, y - 1, z, fluid, new_decay, world, channels, queue).await;
    } else if l >= 0 && (l == 0 || is_blocked(x, y - 1, z, world).await) {
        let dirs = optimal_flow_directions(x, y, z, fluid, world).await;

        let mut j1 = l + step;
        if l >= 8 {
            j1 = 1;
        }
        if j1 >= 8 {
            return;
        }

        for (i, (dx, dz)) in DIRS.iter().enumerate() {
            if dirs[i] {
                flow_into(x + dx, y, z + dz, fluid, j1 as u8, world, channels, queue).await;
            }
        }
    }
}

const MAX_FLOW_DEPTH: i32 = 4;
const FLOW_SCAN_RADIUS: i32 = 1 + MAX_FLOW_DEPTH;
const SCAN_SIDE: usize = (2 * FLOW_SCAN_RADIUS + 1) as usize;
const SCAN_PLANE: usize = SCAN_SIDE * SCAN_SIDE;
const SCAN_LEN: usize = 2 * SCAN_PLANE;

struct FlowScan {
    origin: (i32, i32, i32),
    blocks: [Block; SCAN_LEN],
}
impl FlowScan {
    async fn around(x: i32, y: i32, z: i32, world: &Arc<World>) -> Self {
        let mut positions = Vec::with_capacity(SCAN_LEN);
        for level in [y, y - 1] {
            for dx in -FLOW_SCAN_RADIUS..=FLOW_SCAN_RADIUS {
                for dz in -FLOW_SCAN_RADIUS..=FLOW_SCAN_RADIUS {
                    let clamped = level.clamp(0, 255) as u8;
                    positions.push((x + dx, clamped, z + dz));
                }
            }
        }
        let mut fetched = Vec::with_capacity(SCAN_LEN);
        world.get_many(&positions, &mut fetched).await;

        let mut blocks: [Block; SCAN_LEN] = std::array::from_fn(|_| Block::air());
        for (slot, block) in blocks.iter_mut().zip(fetched) {
            if let Some(block) = block {
                *slot = block;
            }
        }

        Self {
            origin: (x, y, z),
            blocks,
        }
    }

    fn index(level: usize, dx: i32, dz: i32) -> Option<usize> {
        if dx.abs() > FLOW_SCAN_RADIUS || dz.abs() > FLOW_SCAN_RADIUS {
            return None;
        }
        let row = (dx + FLOW_SCAN_RADIUS) as usize;
        let col = (dz + FLOW_SCAN_RADIUS) as usize;
        Some(level * SCAN_PLANE + row * SCAN_SIDE + col)
    }

    fn get(&self, x: i32, y: i32, z: i32) -> Block {
        let (ox, oy, oz) = self.origin;
        let level = match oy - y {
            0 => 0,
            1 => 1,
            _ => {
                debug_assert!(false, "flow scan read y={y}, which is not a scanned level");
                return Block::air();
            }
        };
        match Self::index(level, x - ox, z - oz) {
            Some(i) => self.blocks[i].clone(),
            None => {
                debug_assert!(false, "flow scan read ({x}, {y}, {z}), outside its radius");
                Block::air()
            }
        }
    }

    fn is_blocked(&self, x: i32, y: i32, z: i32) -> bool {
        if !(0..255).contains(&y) {
            return true;
        }
        let b = self.get(x, y, z);
        !(b.is_air() || is_replaceable(b.id) || Fluid::is_fluid(b.id))
    }

    fn passable(&self, x: i32, y: i32, z: i32, fluid: Fluid) -> bool {
        if self.is_blocked(x, y, z) {
            return false;
        }
        let b = self.get(x, y, z);
        if Fluid::same_kind(b.id, fluid.block_id()) {
            !Fluid::is_source(b.id, b.metadata)
        } else {
            true
        }
    }
}

async fn optimal_flow_directions(
    x: i32,
    y: i32,
    z: i32,
    fluid: Fluid,
    world: &Arc<World>,
) -> [bool; 4] {
    let scan = FlowScan::around(x, y, z, world).await;
    optimal_flow_directions_in(x, y, z, fluid, &scan)
}

fn optimal_flow_directions_in(x: i32, y: i32, z: i32, fluid: Fluid, scan: &FlowScan) -> [bool; 4] {
    let mut costs = [1000i32; 4];

    for (i, (dx, dz)) in DIRS.iter().enumerate() {
        let (nx, nz) = (x + dx, z + dz);
        if !scan.passable(nx, y, nz, fluid) {
            continue;
        }
        let below_blocked = scan.is_blocked(nx, y - 1, nz);
        costs[i] = if below_blocked {
            flow_cost(nx, y, nz, 1, i, fluid, scan)
        } else {
            0
        };
    }

    let min = *costs.iter().min().unwrap_or(&1000);
    let mut out = [false; 4];
    for i in 0..4 {
        out[i] = costs[i] == min;
    }
    out
}

fn flow_cost(
    x: i32,
    y: i32,
    z: i32,
    depth: i32,
    from_dir: usize,
    fluid: Fluid,
    scan: &FlowScan,
) -> i32 {
    let mut best = 1000;
    let opposite = match from_dir {
        0 => 1,
        1 => 0,
        2 => 3,
        _ => 2,
    };

    for (i, (dx, dz)) in DIRS.iter().enumerate() {
        if i == opposite {
            continue;
        }
        let (nx, nz) = (x + dx, z + dz);
        if !scan.passable(nx, y, nz, fluid) {
            continue;
        }
        if !scan.is_blocked(nx, y - 1, nz) {
            return depth;
        }
        if depth < MAX_FLOW_DEPTH {
            best = best.min(flow_cost(nx, y, nz, depth + 1, i, fluid, scan));
        }
    }
    best
}

async fn set_block(
    x: i32,
    y: i32,
    z: i32,
    id: u8,
    metadata: u8,
    world: &Arc<World>,
    channels: &Channels,
    queue: &Arc<RwLock<FluidQueue>>,
) {
    world.set(x, y as u8, z, Block::new(id, metadata)).await;
    channels.block_tx.send((x, y, z, id as i32, metadata)).ok();
    queue_fluid_update(x, y, z, queue).await;
}

fn fizz(x: i32, y: i32, z: i32, channels: &Channels) {
    channels
        .sound_tx
        .send((
            "random.fizz",
            x as f64 + 0.5,
            y as f64 + 0.5,
            z as f64 + 0.5,
            0.5,
            63,
        ))
        .ok();
}

#[cfg(test)]
mod queue_tests {
    use super::*;

    #[test]
    fn a_position_is_only_queued_once() {
        let mut q = FluidQueue::new();
        assert!(q.push((1, 2, 3)));
        assert!(!q.push((1, 2, 3)));
        assert!(!q.push((1, 2, 3)));
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn draining_releases_a_position_for_requeueing() {
        let mut q = FluidQueue::new();
        q.push((1, 2, 3));
        assert_eq!(q.drain_batch(10), vec![(1, 2, 3)]);
        assert!(q.order.is_empty());
        assert!(q.push((1, 2, 3)));
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn positions_come_back_in_insertion_order() {
        let mut q = FluidQueue::new();
        for i in 0..5 {
            q.push((i, 0, 0));
        }
        assert_eq!(q.drain_batch(3), vec![(0, 0, 0), (1, 0, 0), (2, 0, 0)]);
        assert_eq!(q.drain_batch(10), vec![(3, 0, 0), (4, 0, 0)]);
    }

    #[test]
    fn draining_more_than_is_queued_is_fine() {
        let mut q = FluidQueue::new();
        q.push((0, 0, 0));
        assert_eq!(q.drain_batch(200).len(), 1);
        assert!(q.drain_batch(200).is_empty());
    }

    #[test]
    fn a_partial_drain_keeps_the_rest_indexed() {
        let mut q = FluidQueue::new();
        for i in 0..5 {
            q.push((i, 0, 0));
        }
        q.drain_batch(2);
        assert!(!q.push((4, 0, 0)), "still-pending position was re-queued");
        assert!(q.push((0, 0, 0)), "drained position could not re-enter");
    }
}

#[cfg(test)]
mod flow_scan_tests {
    use std::sync::Arc;

    use coral_world::{
        blocks::{Block, fluid::Fluid},
        generator::VoidWorldGenerator,
        world::World,
    };

    use super::{
        DIRS, FlowScan, MAX_FLOW_DEPTH, is_blocked, optimal_flow_directions,
        optimal_flow_directions_in,
    };

    const STONE: u8 = 1;
    const WATER: u8 = 9;

    async fn oracle_passable(x: i32, y: i32, z: i32, fluid: Fluid, world: &Arc<World>) -> bool {
        if is_blocked(x, y, z, world).await {
            return false;
        }
        let b = world.get(x, y as u8, z).await;
        if Fluid::same_kind(b.id, fluid.block_id()) {
            !Fluid::is_source(b.id, b.metadata)
        } else {
            true
        }
    }

    async fn oracle_cost(
        x: i32,
        y: i32,
        z: i32,
        depth: i32,
        from_dir: usize,
        fluid: Fluid,
        world: &Arc<World>,
    ) -> i32 {
        let mut best = 1000;
        let opposite = match from_dir {
            0 => 1,
            1 => 0,
            2 => 3,
            _ => 2,
        };
        for (i, (dx, dz)) in DIRS.iter().enumerate() {
            if i == opposite {
                continue;
            }
            let (nx, nz) = (x + dx, z + dz);
            if !oracle_passable(nx, y, nz, fluid, world).await {
                continue;
            }
            if !is_blocked(nx, y - 1, nz, world).await {
                return depth;
            }
            if depth < MAX_FLOW_DEPTH {
                let c = Box::pin(oracle_cost(nx, y, nz, depth + 1, i, fluid, world)).await;
                best = best.min(c);
            }
        }
        best
    }

    async fn oracle_directions(
        x: i32,
        y: i32,
        z: i32,
        fluid: Fluid,
        world: &Arc<World>,
    ) -> [bool; 4] {
        let mut costs = [1000i32; 4];
        for (i, (dx, dz)) in DIRS.iter().enumerate() {
            let (nx, nz) = (x + dx, z + dz);
            if !oracle_passable(nx, y, nz, fluid, world).await {
                continue;
            }
            costs[i] = if is_blocked(nx, y - 1, nz, world).await {
                oracle_cost(nx, y, nz, 1, i, fluid, world).await
            } else {
                0
            };
        }
        let min = *costs.iter().min().unwrap_or(&1000);
        let mut out = [false; 4];
        for i in 0..4 {
            out[i] = costs[i] == min;
        }
        out
    }

    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    fn void_world() -> Arc<World> {
        Arc::new(World::new(Arc::new(VoidWorldGenerator::new())))
    }

    async fn random_terrain(seed: u64, y: i32, hole_in: u64, wall_in: u64) -> Arc<World> {
        let world = void_world();
        let mut rng = Rng(seed | 1);
        let reach = super::FLOW_SCAN_RADIUS + 2;
        for x in -reach..=reach {
            for z in -reach..=reach {
                if rng.below(hole_in) != 0 {
                    world.set(x, (y - 1) as u8, z, Block::new(STONE, 0)).await;
                }
                match rng.below(wall_in) {
                    0 => world.set(x, y as u8, z, Block::new(STONE, 0)).await,
                    1 => world.set(x, y as u8, z, Block::new(WATER, 0)).await,
                    _ => {}
                }
            }
        }
        world.set(0, y as u8, 0, Block::new(WATER, 1)).await;
        world
    }

    #[tokio::test]
    async fn the_prefetched_search_agrees_with_the_one_that_reads_the_world() {
        let fluid = Fluid::STATIONARY_WATER;
        for seed in 1..=60u64 {
            let (hole_in, wall_in) = match seed % 3 {
                0 => (40, 12),
                1 => (12, 6),
                _ => (6, 4),
            };
            let world = random_terrain(seed, 64, hole_in, wall_in).await;

            let expected = oracle_directions(0, 64, 0, fluid, &world).await;
            let actual = optimal_flow_directions(0, 64, 0, fluid, &world).await;

            assert_eq!(actual, expected, "seed {seed} disagreed");
        }
    }

    #[tokio::test]
    async fn a_sealed_floor_forces_the_search_to_its_full_depth() {
        let world = void_world();
        let reach = super::FLOW_SCAN_RADIUS + 2;
        for x in -reach..=reach {
            for z in -reach..=reach {
                world.set(x, 63, z, Block::new(STONE, 0)).await;
            }
        }
        world.set(0, 64, 0, Block::new(WATER, 1)).await;

        let fluid = Fluid::STATIONARY_WATER;
        let expected = oracle_directions(0, 64, 0, fluid, &world).await;
        let actual = optimal_flow_directions(0, 64, 0, fluid, &world).await;

        assert_eq!(actual, expected);
        assert_eq!(actual, [true; 4], "a flat sealed floor spreads every way");
    }

    #[tokio::test]
    async fn the_scan_covers_everything_the_search_reads() {
        let world = void_world();
        let reach = super::FLOW_SCAN_RADIUS + 2;
        for x in -reach..=reach {
            for z in -reach..=reach {
                world.set(x, 63, z, Block::new(STONE, 0)).await;
            }
        }
        world.set(0, 64, 0, Block::new(WATER, 1)).await;

        let scan = FlowScan::around(0, 64, 0, &world).await;
        optimal_flow_directions_in(0, 64, 0, Fluid::STATIONARY_WATER, &scan);
    }

    #[tokio::test]
    async fn the_search_still_picks_the_nearest_hole() {
        let world = void_world();
        let reach = super::FLOW_SCAN_RADIUS + 2;
        for x in -reach..=reach {
            for z in -reach..=reach {
                world.set(x, 63, z, Block::new(STONE, 0)).await;
            }
        }
        world.set(3, 63, 0, Block::air()).await;
        world.set(0, 64, 0, Block::new(WATER, 1)).await;

        let dirs = optimal_flow_directions(0, 64, 0, Fluid::STATIONARY_WATER, &world).await;
        assert_eq!(dirs, [false, true, false, false], "water ignored the hole");
    }

    #[tokio::test]
    async fn a_scan_at_the_world_floor_does_not_read_below_it() {
        let world = void_world();
        world.set(0, 0, 0, Block::new(WATER, 1)).await;

        let dirs = optimal_flow_directions(0, 0, 0, Fluid::STATIONARY_WATER, &world).await;
        assert_eq!(dirs, [true; 4]);
    }
}
