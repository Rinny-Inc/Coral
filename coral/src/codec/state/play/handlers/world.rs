use std::time::Instant;

use coral_protocol::packets::play::{
    block::{DigStatus, HeldItemChange, PlayerBlockPlacement, PlayerDig},
    inventory::ItemStack,
};
use coral_server::{
    bounding_box::EntityBounds,
    entity_tracker::TrackedEntity,
    experience::{self, XpOrb},
    items::{drop_physics::break_drop_velocity, drops::block_drop, ids::EMPTY_HAND},
    mining::{break_time_ticks, can_harvest},
    player::registry::next_entity_id,
    projectile::{Projectile, ProjectileKind},
    reach,
};
use coral_types::GameMode;
use coral_world::blocks::{Block, placement_metadata};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use crate::codec::{
    Codec, PlayerState, send_packet,
    state::play::{
        PlayCtx, block_break_sound, drop_item_in_world, handlers::Flow, interact,
        refuse_out_of_reach,
    },
};

async fn out_of_reach(
    framed: &mut Framed<TcpStream, Codec>,
    state: &PlayerState,
    ctx: &PlayCtx<'_>,
    bx: i32,
    by: i32,
    bz: i32,
) -> bool {
    if let Some(me) = ctx.player_registry.get(&state.uuid).await
        && !reach::can_reach_block(
            (me.x, me.y, me.z),
            state.is_sneaking,
            state.gamemode,
            bx,
            by,
            bz,
        )
    {
        refuse_out_of_reach(framed, ctx.world, &state.name, bx, by, bz).await;
        return true;
    }
    false
}

pub(in crate::codec::state::play) async fn player_dig(
    dig: PlayerDig,
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    if dig.status.needs_reach_check()
        && out_of_reach(framed, state, ctx, dig.x, dig.y as i32, dig.z).await
    {
        return Flow::Continue;
    }

    match dig.status {
        DigStatus::StartDig => start_dig(&dig, state, ctx).await,
        DigStatus::CancelDig => cancel_dig(state, ctx),
        DigStatus::FinishDig => finish_dig(framed, state, ctx).await,
        DigStatus::DropItem(is_itemstack) => drop_item(is_itemstack, framed, state, ctx).await,
        DigStatus::ShootOrFinishEating => shoot_or_finish_eating(framed, state, ctx).await,
    }
}

async fn start_dig(dig: &PlayerDig, state: &mut PlayerState, ctx: &PlayCtx<'_>) -> Flow {
    let block = ctx.world.get(dig.x, dig.y, dig.z).await;

    if state.gamemode == GameMode::Creative {
        ctx.world.set(dig.x, dig.y, dig.z, Block::air()).await;
        ctx.channels
            .block_tx
            .send((dig.x, dig.y as i32, dig.z, 0, 0))
            .ok();
        ctx.channels
            .sound_tx
            .send((
                block_break_sound(block.id),
                dig.x as f64 + 0.5,
                dig.y as f64 + 0.5,
                dig.z as f64 + 0.5,
                1.0,
                63,
            ))
            .ok();

        ctx.tile_entities
            .write()
            .await
            .remove(&(dig.x, dig.y as i32, dig.z));
        return Flow::Continue;
    }
    let in_water = state.is_head_in_water(ctx.world).await;
    let required = break_time_ticks(
        ctx.item_registry,
        ctx.block_registry,
        state.held_item.id(),
        block.id,
        in_water,
        state.was_on_ground,
    );

    state.breaking_block = Some((dig.x, dig.y as i32, dig.z));
    state.breaking_started_tick = state.tick_count;
    state.breaking_required_ticks = required;
    ctx.channels
        .break_tx
        .send((state.entity_id, dig.x, dig.y as i32, dig.z, 0))
        .ok();
    Flow::Continue
}

fn cancel_dig(state: &mut PlayerState, ctx: &PlayCtx<'_>) -> Flow {
    if let Some((bx, by, bz)) = state.breaking_block.take() {
        ctx.channels
            .break_tx
            .send((state.entity_id, bx, by, bz, 255))
            .ok();
    }
    Flow::Continue
}

async fn finish_dig(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    if state.gamemode != GameMode::Survival {
        return Flow::Continue;
    }
    if let Some((bx, by, bz)) = state.breaking_block.take() {
        let block = ctx.world.get(bx, by as u8, bz).await;

        let in_water = state.is_head_in_water(ctx.world).await;
        let required_ticks = break_time_ticks(
            ctx.item_registry,
            ctx.block_registry,
            state.held_item.id(),
            block.id,
            in_water,
            state.was_on_ground,
        );
        let elapsed = (state.tick_count - state.breaking_started_tick).max(0) as u32;

        if elapsed < required_ticks {
            ctx.channels
                .block_tx
                .send((bx, by, bz, block.id as i32, block.metadata))
                .ok();
            state.breaking_block = Some((bx, by, bz));
            return Flow::Continue;
        }
        ctx.world.set(bx, by as u8, bz, Block::air()).await;
        ctx.channels.block_tx.send((bx, by, bz, 0, 0)).ok();
        ctx.channels
            .break_tx
            .send((state.entity_id, bx, by, bz, 10))
            .ok();
        ctx.channels
            .particle_tx
            .send((
                state.entity_id,
                37,
                bx as f32 + 0.5,
                by as f32 + 0.5,
                bz as f32 + 0.5,
                0.3,
                0.3,
                0.3,
                block.id as f32,
                8,
                [block.id as i32 | ((block.metadata as i32) << 12), 0],
            ))
            .ok();
        ctx.channels
            .sound_tx
            .send((
                block_break_sound(block.id),
                bx as f64 + 0.5,
                by as f64 + 0.5,
                bz as f64 + 0.5,
                1.0,
                63,
            ))
            .ok();

        ctx.tile_entities.write().await.remove(&(bx, by, bz));

        if !block.is_air() && block.id > 0 {
            let can_drop = can_harvest(
                ctx.item_registry,
                ctx.block_registry,
                state.held_item.id(),
                block.id,
            );

            if can_drop
                && let Some((drop_id, drop_count, drop_metadata)) =
                    block_drop(block.id, block.metadata)
            {
                let drop_eid = next_entity_id();
                let x = bx as f64 + 0.5;
                let y = by as f64 + 0.5;
                let z = bz as f64 + 0.5;
                let (vx, vy, vz) = break_drop_velocity();

                ctx.entity_tracker.write().await.track(TrackedEntity::item(
                    drop_eid,
                    x,
                    y,
                    z,
                    ctx.config.tracking.item,
                ));
                ctx.item_spawn_times
                    .write()
                    .await
                    .insert(drop_eid, Instant::now());
                ctx.item_positions.write().await.insert((
                    drop_eid,
                    x,
                    y,
                    z,
                    drop_id,
                    drop_count,
                    drop_metadata,
                ));
                ctx.channels
                    .item_tx
                    .send((
                        drop_eid,
                        x,
                        y,
                        z,
                        drop_id,
                        drop_count,
                        drop_metadata,
                        vx,
                        vy,
                        vz,
                    ))
                    .ok();
            }

            let xp = experience::xp_for_block(block.id);
            if xp > 0 {
                let orb_eid = next_entity_id();
                let ox = bx as f64 + 0.5;
                let oy = by as f64 + 0.5;
                let oz = bz as f64 + 0.5;

                ctx.entity_tracker
                    .write()
                    .await
                    .track(TrackedEntity::experience_orb(
                        orb_eid,
                        ox,
                        oy,
                        oz,
                        ctx.config.tracking.experience_orb,
                    ));

                ctx.xp_orbs.write().await.push(XpOrb {
                    entity_id: orb_eid,
                    x: ox,
                    y: oy,
                    z: oz,
                    vy: 0.2,
                    amount: xp,
                    ticks_alive: 0,
                });
                ctx.channels
                    .xp_orb_spawn_tx
                    .send((orb_eid, ox, oy, oz, xp))
                    .ok();
            }
        }
    }
    let broke = state.damage_item(1, ctx.item_registry);
    state
        .sync_held_slot_after_damage(
            framed,
            ctx.player_registry,
            ctx.item_registry,
            ctx.channels,
            broke,
        )
        .await;
    Flow::Continue
}

async fn drop_item(
    is_itemstack: bool,
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    let hotbar_slot = state.held_slot as usize;

    if state.inventory.slots[hotbar_slot].is_none() {
        return Flow::Continue;
    }

    let item = if is_itemstack {
        state.inventory.slots[hotbar_slot].take()
    } else if let Some(slot) = state.inventory.slots[hotbar_slot].as_mut() {
        slot.count -= 1;
        let dropped = ItemStack {
            item_id: slot.item_id,
            count: 1,
            metadata: slot.metadata,
            durability: 0,
        };
        if slot.count == 0 {
            state.inventory.slots[hotbar_slot] = None;
        }
        Some(dropped)
    } else {
        None
    };

    if let Some(dropped) = item {
        state
            .sync_hotbar_slot(framed, ctx.player_registry, ctx.item_registry, ctx.channels)
            .await;

        drop_item_in_world(
            dropped,
            state.entity_id,
            ctx.player_registry,
            ctx.channels,
            ctx.item_spawn_times,
            ctx.entity_tracker,
            ctx.item_positions,
            ctx.config.tracking.item,
        )
        .await;

        send_packet(
            framed,
            HeldItemChange {
                slot: state.held_slot as i16,
            },
        )
        .await;
    }
    Flow::Continue
}

async fn shoot_or_finish_eating(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    if let Some(charge_start) = state.bow_charging.take()
        && state.held_item.id() == coral_server::items::ids::BOW
    // TODO: In the future identify magic numbers!
    {
        let charge_secs = charge_start.elapsed().as_secs_f32().min(1.0);

        if charge_secs < 0.1 {
            return Flow::Continue;
        }

        let power = ((charge_secs * charge_secs + charge_secs * 2.0) / 3.0).clamp(0.0, 1.0);

        if let Some(p) = ctx.player_registry.get(&state.uuid).await {
            let (dx, dy, dz) = p.get_head_direction();
            let speed = power as f64 * 3.0;
            let arrow_eid = next_entity_id();

            let proj = Projectile {
                entity_id: arrow_eid,
                owner_entity_id: state.entity_id,
                kind: ProjectileKind::Arrow,
                x: p.x,
                y: p.y + EntityBounds::player(p.is_sneaking).height,
                z: p.z,
                vx: dx * speed + state.velocity.0,
                vy: dy * speed + state.velocity.1.max(0.0),
                vz: dz * speed + state.velocity.2,
                ticks_alive: 0,
                left_owner: false,
            };

            ctx.projectiles.write().await.push(proj.clone());
            ctx.channels
                .projectile_spawn_tx
                .send((
                    arrow_eid,
                    state.entity_id,
                    ProjectileKind::Arrow,
                    proj.x,
                    proj.y,
                    proj.z,
                    proj.vx,
                    proj.vy,
                    proj.vz,
                ))
                .ok();

            ctx.channels
                .sound_tx
                .send(("random.bow", p.x, p.y, p.z, 1.0, 63))
                .ok();

            if state.gamemode == GameMode::Survival {
                state.consume_arrow_from_inventory();
            }
        }
        return Flow::Continue;
    }
    if state.held_item.id() == coral_server::items::ids::SPLASH_POTION {
        let meta = state.inventory.slots[state.held_slot as usize]
            .as_ref()
            .map(|s| s.metadata)
            .unwrap_or(0);
        let is_splash = (meta & 0x4000i16) != 0;
        if is_splash && let Some(p) = ctx.player_registry.get(&state.uuid).await {
            let (dx, dy, dz) = p.get_head_direction();
            let speed = 0.5;
            let proj_eid = next_entity_id();

            let proj = Projectile {
                entity_id: proj_eid,
                owner_entity_id: state.entity_id,
                kind: ProjectileKind::SplashPotion(meta),
                x: p.x,
                y: p.y + 1.5,
                z: p.z,
                vx: dx * speed,
                vy: dy * speed + 0.1,
                vz: dz * speed,
                ticks_alive: 0,
                left_owner: false,
            };

            ctx.projectiles.write().await.push(proj.clone());
            ctx.channels
                .projectile_spawn_tx
                .send((
                    proj_eid,
                    state.entity_id,
                    ProjectileKind::SplashPotion(meta),
                    proj.x,
                    proj.y,
                    proj.z,
                    proj.vx,
                    proj.vy,
                    proj.vz,
                ))
                .ok();

            state
                .consume_held_one(framed, ctx.player_registry, ctx.item_registry, ctx.channels)
                .await;
        }
    }
    Flow::Continue
}

pub(in crate::codec::state::play) async fn player_block_placement(
    place: PlayerBlockPlacement,
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    let Some(face) = place.face.clone() else {
        if interact::try_with_item(
            state,
            ctx.item_registry,
            ctx.player_registry,
            ctx.projectiles,
            ctx.channels,
        )
        .await
        {
            return Flow::Continue;
        }
        if interact::try_with_item_on_block(
            framed,
            &place,
            state,
            ctx.player_registry,
            ctx.item_registry,
            ctx.world,
            ctx.tile_entities,
            ctx.fluid_queue,
            ctx.channels,
            None,
        )
        .await
        {
            return Flow::Continue;
        }
        return Flow::Continue;
    };

    if out_of_reach(framed, state, ctx, place.x, place.y as i32, place.z).await {
        return Flow::Continue;
    }

    if interact::try_with_block(
        framed,
        &place,
        state,
        ctx.player_registry,
        ctx.item_registry,
        ctx.world,
        ctx.tile_entities,
        ctx.fluid_queue,
        ctx.channels,
    )
    .await
        || place.held_item_id == EMPTY_HAND
        || state.gamemode >= GameMode::Adventure
    {
        return Flow::Continue;
    }
    let (tx, ty, tz) = face.to_placement(place.x, place.y as i32, place.z);

    if !(0..=255).contains(&ty) {
        return Flow::Continue;
    }

    let block_id = place.held_item_id as i32;
    if block_id <= 0 || block_id > 255 {
        return Flow::Continue;
    }

    let block_meta = {
        let item_meta = state.inventory.slots[state.held_slot as usize]
            .as_ref()
            .map(|s| s.metadata as u8)
            .unwrap_or(0);

        let yaw = ctx
            .player_registry
            .get(&state.uuid)
            .await
            .map(|p| p.yaw)
            .unwrap_or(0.0);

        let cursor_y = place.cursor_y as f32 / 16.0; // 0-15 -> 0.0-1.0

        placement_metadata(block_id as u8, item_meta, face, yaw, cursor_y)
    };

    if state.gamemode == GameMode::Survival {
        state
            .consume_held_one(framed, ctx.player_registry, ctx.item_registry, ctx.channels)
            .await;
    }

    ctx.world
        .blocks
        .set(tx, ty as u8, tz, Block::new(block_id as u8, block_meta))
        .await;
    ctx.channels
        .block_tx
        .send((tx, ty, tz, block_id, block_meta))
        .ok();
    ctx.channels
        .sound_tx
        .send((
            block_break_sound(block_id as u8),
            tx as f64 + 0.5,
            ty as f64 + 0.5,
            tz as f64 + 0.5,
            1.0,
            63,
        ))
        .ok();
    Flow::Continue
}
