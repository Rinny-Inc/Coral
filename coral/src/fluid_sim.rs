use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use coral_world::blocks::{Block, WorldBlocks};
use tokio::sync::RwLock;
use tokio::time::interval;

use coral_world::blocks::fluid::{Fluid, FluidKind, is_replaceable};
use coral_world::generator::FlatWorldGenerator;

use crate::Channels;

const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

pub async fn queue_fluid_update(
    x: i32,
    y: i32,
    z: i32,
    fluid_queue: &Arc<RwLock<VecDeque<(i32, i32, i32)>>>,
) {
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
        if (0..=255).contains(&pos.1) && !q.contains(&pos) {
            q.push_back(pos);
        }
    }
}

pub fn spawn_fluid_task(
    fluid_queue: Arc<RwLock<VecDeque<(i32, i32, i32)>>>,
    world_blocks: Arc<WorldBlocks>,
    generator: Arc<FlatWorldGenerator>,
    channels: Channels,
) {
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(250)); // 5 game ticks
        let mut tick: u64 = 0;

        loop {
            ticker.tick().await;
            tick += 5;

            let batch: Vec<(i32, i32, i32)> = {
                let mut q = fluid_queue.write().await;
                let n = q.len().min(200);
                q.drain(..n).collect()
            };

            for (x, y, z) in batch {
                println!("[Q] {} {} {}", x, y, z);
                if !(0..=255).contains(&y) {
                    continue;
                }
                let block = world_blocks.get(x, y as u8, z, &generator).await;
                let Some(fluid) = Fluid::from_block_id(block.id) else {
                    continue;
                };

                if tick % fluid.spread_rate_ticks() != 0 {
                    fluid_queue.write().await.push_back((x, y, z));
                    continue;
                }

                update_tick(
                    x,
                    y,
                    z,
                    fluid,
                    block.metadata,
                    &world_blocks,
                    &generator,
                    &channels,
                    &fluid_queue,
                )
                .await;
            }
        }
    });
}

fn decay_step(fluid: Fluid) -> i32 {
    match fluid.kind {
        FluidKind::Water => 1,
        FluidKind::Lava => 2,
    }
}

fn flow_decay_of(fluid: Fluid, id: u8, metadata: u8) -> i32 {
    if Fluid::same_kind(id, fluid.block_id()) {
        metadata as i32
    } else {
        -1
    }
}

fn smallest_flow_decay(neighbor_decay: i32, current: i32, adjacent_sources: &mut u8) -> i32 {
    let mut d = neighbor_decay;
    if d < 0 {
        return current;
    }
    if d == 0 {
        *adjacent_sources += 1;
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

async fn can_flow_into(
    x: i32,
    y: i32,
    z: i32,
    world_blocks: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
) -> bool {
    if !(0..=255).contains(&y) {
        return false;
    }
    let b = world_blocks.get(x, y as u8, z, generator).await;
    if b.is_air() || is_replaceable(b.id) {
        return true;
    }
    false
}

async fn update_tick(
    x: i32,
    y: i32,
    z: i32,
    fluid: Fluid,
    metadata: u8,
    world_blocks: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
    channels: &Channels,
    queue: &Arc<RwLock<VecDeque<(i32, i32, i32)>>>,
) {
    let flowing = fluid.flowing_variant();
    let step = decay_step(fluid);

    let is_stationary = !Fluid::from_block_id(fluid.block_id())
        .map(|f| f.flowing)
        .unwrap_or(true);
    let mut l: i32 = if is_stationary { 0 } else { metadata as i32 };

    if l > 0 {
        let mut adjacent_sources: u8 = 0;
        let mut k1: i32 = -100;

        for (dx, dz) in DIRS {
            let n = world_blocks.get(x + dx, y as u8, z + dz, generator).await;
            let nd = flow_decay_of(fluid, n.id, n.metadata);
            k1 = smallest_flow_decay(nd, k1, &mut adjacent_sources);
        }

        let mut j1 = k1 + step;
        if j1 >= 8 || k1 < 0 {
            j1 = -1;
        }

        if y < 255 {
            let above = world_blocks.get(x, (y + 1) as u8, z, generator).await;
            let ad = flow_decay_of(fluid, above.id, above.metadata);
            if ad >= 0 {
                j1 = if ad >= 8 { ad } else { ad + 8 };
            }
        }

        if adjacent_sources >= 2 && fluid.kind == FluidKind::Water && y > 0 {
            let below = world_blocks.get(x, (y - 1) as u8, z, generator).await;
            let below_solid =
                !below.is_air() && !is_replaceable(below.id) && !Fluid::is_fluid(below.id);
            let below_source = Fluid::same_kind(below.id, fluid.block_id())
                && Fluid::is_source(below.id, below.metadata);
            if below_solid || below_source {
                j1 = 0; // becomes a source
            }
        }

        if j1 != l {
            l = j1;
            if j1 < 0 {
                println!(
                    "[DRAIN] setting air at {} {} {} (was meta {})",
                    x, y, z, metadata
                );
                world_blocks
                    .set(x, y as u8, z, Block::air(), generator)
                    .await;
                let check = world_blocks.get(x, y as u8, z, generator).await;
                println!("[DRAIN] readback at {} {} {} -> id={}", x, y, z, check.id);
                channels.block_tx.send((x, y, z, 0, 0)).ok();
                queue_fluid_update(x, y, z, queue).await;
                return;
            } else {
                world_blocks
                    .set(
                        x,
                        y as u8,
                        z,
                        Block::new(flowing.block_id(), j1 as u8),
                        generator,
                    )
                    .await;
                channels
                    .block_tx
                    .send((x, y, z, flowing.block_id() as i32, j1 as u8))
                    .ok();
                queue_fluid_update(x, y, z, queue).await;
            }
        }
    }

    if y > 0 {
        let below = world_blocks.get(x, (y - 1) as u8, z, generator).await;

        if let Some(other) = Fluid::from_block_id(below.id) {
            if other.kind != fluid.kind {
                let result =
                    fluid_interaction(fluid, other, Fluid::is_source(below.id, below.metadata));
                world_blocks
                    .set(x, (y - 1) as u8, z, Block::new(result, 0), generator)
                    .await;
                channels.block_tx.send((x, y - 1, z, result as i32, 0)).ok();
                fizz(x, y - 1, z, channels);
                queue_fluid_update(x, y - 1, z, queue).await;
                return;
            }
        } else if can_flow_into(x, y - 1, z, world_blocks, generator).await {
            let new_meta = if l >= 8 { l as u8 } else { (l + 8) as u8 };
            world_blocks
                .set(
                    x,
                    (y - 1) as u8,
                    z,
                    Block::new(flowing.block_id(), new_meta),
                    generator,
                )
                .await;
            channels
                .block_tx
                .send((x, y - 1, z, flowing.block_id() as i32, new_meta))
                .ok();
            queue_fluid_update(x, y - 1, z, queue).await;
            return;
        }
    }

    let mut j1 = l + step;
    if l >= 8 {
        j1 = 1;
    }
    if j1 >= 8 {
        return;
    }

    let dirs = optimal_flow_directions(x, y, z, fluid, world_blocks, generator).await;

    for (i, (dx, dz)) in DIRS.iter().enumerate() {
        if !dirs[i] {
            continue;
        }
        let (nx, nz) = (x + dx, z + dz);
        let n = world_blocks.get(nx, y as u8, nz, generator).await;

        if let Some(other) = Fluid::from_block_id(n.id) {
            if other.kind != fluid.kind {
                let result = fluid_interaction(fluid, other, Fluid::is_source(n.id, n.metadata));
                world_blocks
                    .set(nx, y as u8, nz, Block::new(result, 0), generator)
                    .await;
                channels.block_tx.send((nx, y, nz, result as i32, 0)).ok();
                fizz(nx, y, nz, channels);
                queue_fluid_update(nx, y, nz, queue).await;
            }
            continue;
        }

        if can_flow_into(nx, y, nz, world_blocks, generator).await {
            world_blocks
                .set(
                    nx,
                    y as u8,
                    nz,
                    Block::new(flowing.block_id(), j1 as u8),
                    generator,
                )
                .await;
            channels
                .block_tx
                .send((nx, y, nz, flowing.block_id() as i32, j1 as u8))
                .ok();
            queue_fluid_update(nx, y, nz, queue).await;
        }
    }
}

async fn optimal_flow_directions(
    x: i32,
    y: i32,
    z: i32,
    fluid: Fluid,
    world_blocks: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
) -> [bool; 4] {
    let mut costs = [1000i32; 4];

    for (i, (dx, dz)) in DIRS.iter().enumerate() {
        let (nx, nz) = (x + dx, z + dz);

        if !can_flow_into(nx, y, nz, world_blocks, generator).await {
            continue;
        }

        if can_flow_into(nx, y - 1, nz, world_blocks, generator).await {
            costs[i] = 0;
        } else {
            costs[i] = flow_cost(nx, y, nz, 1, i, fluid, world_blocks, generator).await;
        }
    }

    let min = *costs.iter().min().unwrap_or(&1000);
    let mut out = [false; 4];
    for i in 0..4 {
        out[i] = costs[i] == min && min < 1000;
    }
    if !out.iter().any(|&b| b) {
        for (i, (dx, dz)) in DIRS.iter().enumerate() {
            out[i] = can_flow_into(x + dx, y, z + dz, world_blocks, generator).await;
        }
    }
    out
}

async fn flow_cost(
    x: i32,
    y: i32,
    z: i32,
    accumulated: i32,
    from_dir: usize,
    fluid: Fluid,
    world_blocks: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
) -> i32 {
    let mut cost = 1000;

    for (i, (dx, dz)) in DIRS.iter().enumerate() {
        let opposite = match from_dir {
            0 => 1,
            1 => 0,
            2 => 3,
            _ => 2,
        };
        if i == opposite {
            continue;
        }

        let (nx, nz) = (x + dx, z + dz);
        if !can_flow_into(nx, y, nz, world_blocks, generator).await {
            continue;
        }
        if can_flow_into(nx, y - 1, nz, world_blocks, generator).await {
            return accumulated; // found a drop
        }
        if accumulated < 4 {
            let c = Box::pin(flow_cost(
                nx,
                y,
                nz,
                accumulated + 1,
                i,
                fluid,
                world_blocks,
                generator,
            ))
            .await;
            cost = cost.min(c);
        }
    }

    cost
}

fn fluid_interaction(_a: Fluid, b: Fluid, b_is_source: bool) -> u8 {
    if b.kind == FluidKind::Lava && b_is_source {
        49 // obsidian
    } else {
        4 // cobblestone
    }
}

fn fizz(x: i32, y: i32, z: i32, channels: &Channels) {
    channels
        .sound_tx
        .send((
            "random.fizz".to_string(),
            x as f64 + 0.5,
            y as f64 + 0.5,
            z as f64 + 0.5,
            0.5,
            63,
        ))
        .ok();
}
