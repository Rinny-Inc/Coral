use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;

use coral_world::blocks::{Block, WorldBlocks};
use tokio::sync::RwLock;
use tokio::time::interval;

use coral_world::blocks::fluid::{Fluid, FluidKind, is_replaceable};
use coral_world::generator::FlatWorldGenerator;

use crate::{Channels, TicksExt};

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
        let mut ticker = interval(Duration::from_ticks(5));
        let mut tick: u64 = 0;
        let mut shutdown_rx = channels.shutdown_tx.subscribe();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    tick += 5;

                    let batch: Vec<(i32, i32, i32)> = {
                        let mut q = fluid_queue.write().await;
                        let n = q.len().min(200);
                        q.drain(..n).collect()
                    };

                    for (x, y, z) in batch {
                        if !(0..=255).contains(&y) {
                            continue;
                        }
                        let block = world_blocks.get(x, y as u8, z, &generator).await;
                        let Some(fluid) = Fluid::from_block_id(block.id) else {
                            continue;
                        };

                        let rate = match fluid.kind {
                            FluidKind::Water => 5,
                            FluidKind::Lava => 30,
                        };
                        if !tick.is_multiple_of(rate) {
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
                Ok(()) = shutdown_rx.recv() => {
                    println!("[Fluid] shutting down");
                    break;
                }
            }
        }
    });
}

async fn decay_at(
    fluid: Fluid,
    x: i32,
    y: i32,
    z: i32,
    wb: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
) -> i32 {
    if !(0..=255).contains(&y) {
        return -1;
    }
    let b = wb.get(x, y as u8, z, generator).await;
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

async fn is_blocked(
    x: i32,
    y: i32,
    z: i32,
    wb: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
) -> bool {
    if !(0..=255).contains(&y) {
        return true;
    }
    let b = wb.get(x, y as u8, z, generator).await;
    !(b.is_air() || is_replaceable(b.id) || Fluid::is_fluid(b.id))
}

async fn can_flow_into(
    fluid: Fluid,
    x: i32,
    y: i32,
    z: i32,
    wb: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
) -> bool {
    if !(0..=255).contains(&y) {
        return false;
    }
    let b = wb.get(x, y as u8, z, generator).await;
    if Fluid::same_kind(b.id, fluid.block_id()) {
        return false;
    }
    if Fluid::is_lava(b.id) {
        return false;
    }
    !is_blocked(x, y, z, wb, generator).await
}

async fn flow_into(
    x: i32,
    y: i32,
    z: i32,
    fluid: Fluid,
    decay: u8,
    wb: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
    channels: &Channels,
    queue: &Arc<RwLock<VecDeque<(i32, i32, i32)>>>,
) {
    if !can_flow_into(fluid, x, y, z, wb, generator).await {
        return;
    }
    if fluid.kind == FluidKind::Lava {
        fizz(x, y, z, channels);
    }
    let flowing = fluid.flowing_variant();
    set_block(x, y, z, flowing.block_id(), decay, wb, channels, queue).await;
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
    wb: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
) -> bool {
    if y == 0 {
        return true;
    }
    let below = wb.get(x, (y - 1) as u8, z, generator).await;
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
    wb: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
) -> Option<u8> {
    if fluid.kind != FluidKind::Lava {
        return None;
    }
    let mut water_adjacent = false;
    for (dx, dz) in DIRS {
        let n = wb.get(x + dx, y as u8, z + dz, generator).await;
        if Fluid::is_water(n.id) {
            water_adjacent = true;
            break;
        }
    }
    if !water_adjacent && y < 255 {
        let above = wb.get(x, (y + 1) as u8, z, generator).await;
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
    wb: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
    channels: &Channels,
    queue: &Arc<RwLock<VecDeque<(i32, i32, i32)>>>,
) {
    if let Some(converted) =
        check_lava_water_reaction(x, y, z, fluid, metadata, wb, generator).await
    {
        set_block(x, y, z, converted, 0, wb, channels, queue).await;
        fizz(x, y, z, channels);
        return;
    }

    let step = decay_step(fluid);
    let mut l: i32 = metadata as i32;

    if l > 0 {
        let mut sources: u8 = 0;
        let mut k1: i32 = -100;
        for (dx, dz) in DIRS {
            let nd = decay_at(fluid, x + dx, y, z + dz, wb, generator).await;
            k1 = fold_min_decay(nd, k1, &mut sources);
        }

        let mut j1 = k1 + step;
        if j1 >= 8 || k1 < 0 {
            j1 = -1;
        }

        let above = decay_at(fluid, x, y + 1, z, wb, generator).await;
        if above >= 0 {
            j1 = if above >= 8 { above } else { above + 8 };
        }

        if sources >= 2
            && fluid.kind == FluidKind::Water
            && buildable_or_source_below(x, y, z, fluid, wb, generator).await
        {
            j1 = 0;
        }

        if j1 != l {
            l = j1;
            if j1 < 0 {
                set_block(x, y, z, 0, 0, wb, channels, queue).await; // the drain
                return;
            } else {
                set_block(
                    x,
                    y,
                    z,
                    fluid.flowing_variant().block_id(),
                    j1 as u8,
                    wb,
                    channels,
                    queue,
                )
                .await;
            }
        }
    }
    if y > 0 && can_flow_into(fluid, x, y - 1, z, wb, generator).await {
        if fluid.kind == FluidKind::Lava {
            let below = wb.get(x, (y - 1) as u8, z, generator).await;
            if Fluid::is_water(below.id) {
                set_block(x, y - 1, z, 1 /* stone */, 0, wb, channels, queue).await;
                fizz(x, y - 1, z, channels);
                return;
            }
        }
        let new_decay = if l >= 8 { l as u8 } else { (l + 8) as u8 };
        flow_into(
            x,
            y - 1,
            z,
            fluid,
            new_decay,
            wb,
            generator,
            channels,
            queue,
        )
        .await;
    } else if l >= 0 && (l == 0 || is_blocked(x, y - 1, z, wb, generator).await) {
        let dirs = optimal_flow_directions(x, y, z, fluid, wb, generator).await;

        let mut j1 = l + step;
        if l >= 8 {
            j1 = 1;
        }
        if j1 >= 8 {
            return;
        }

        for (i, (dx, dz)) in DIRS.iter().enumerate() {
            if dirs[i] {
                flow_into(
                    x + dx,
                    y,
                    z + dz,
                    fluid,
                    j1 as u8,
                    wb,
                    generator,
                    channels,
                    queue,
                )
                .await;
            }
        }
    }
}

async fn passable(
    x: i32,
    y: i32,
    z: i32,
    fluid: Fluid,
    wb: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
) -> bool {
    if is_blocked(x, y, z, wb, generator).await {
        return false;
    }
    let b = wb.get(x, y as u8, z, generator).await;
    if Fluid::same_kind(b.id, fluid.block_id()) {
        !Fluid::is_source(b.id, b.metadata)
    } else {
        true
    }
}

async fn optimal_flow_directions(
    x: i32,
    y: i32,
    z: i32,
    fluid: Fluid,
    wb: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
) -> [bool; 4] {
    let mut costs = [1000i32; 4];

    for (i, (dx, dz)) in DIRS.iter().enumerate() {
        let (nx, nz) = (x + dx, z + dz);
        if !passable(nx, y, nz, fluid, wb, generator).await {
            continue;
        }
        let below_blocked = is_blocked(nx, y - 1, nz, wb, generator).await;
        costs[i] = if below_blocked {
            flow_cost(nx, y, nz, 1, i, fluid, wb, generator).await
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

async fn flow_cost(
    x: i32,
    y: i32,
    z: i32,
    depth: i32,
    from_dir: usize,
    fluid: Fluid,
    wb: &Arc<WorldBlocks>,
    generator: &Arc<FlatWorldGenerator>,
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
        if !passable(nx, y, nz, fluid, wb, generator).await {
            continue;
        }
        if !is_blocked(nx, y - 1, nz, wb, generator).await {
            return depth;
        }
        if depth < 4 {
            let c = Box::pin(flow_cost(nx, y, nz, depth + 1, i, fluid, wb, generator)).await;
            best = best.min(c);
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
    wb: &Arc<WorldBlocks>,
    channels: &Channels,
    queue: &Arc<RwLock<VecDeque<(i32, i32, i32)>>>,
) {
    wb.set(x, y as u8, z, Block::new(id, metadata)).await;
    channels.block_tx.send((x, y, z, id as i32, metadata)).ok();
    queue_fluid_update(x, y, z, queue).await;
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
