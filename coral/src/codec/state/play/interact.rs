use std::{
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering::Relaxed},
    },
    time::Instant,
};

use coral_protocol::packets::play::{
    block::PlayerBlockPlacement,
    chat::{
        ChatMessageOut,
        builder::{ChatBuilder, ChatColor},
    },
    entity::UseBed,
};
use coral_server::{
    items::ItemRegistry,
    player::registry::{PlayerRegistry, next_entity_id},
    projectile::{Projectile, ProjectileKind},
};
use coral_types::look_direction;
use coral_world::{blocks::WorldBlocks, generator::FlatWorldGenerator};
use tokio::{net::TcpStream, sync::RwLock};
use tokio_util::codec::Framed;

use crate::{
    Channels,
    codec::{Codec, PlayerState, send_packet},
};

/// Return true if the itme interaction went well
pub async fn try_with_item(
    place: &PlayerBlockPlacement,
    state: &mut PlayerState,
    item_registry: &Arc<ItemRegistry>,
    player_registry: &Arc<PlayerRegistry>,
    projectiles: &Arc<RwLock<Vec<Projectile>>>,
    channels: &Channels,
) -> bool {
    if place.face != 255 {
        return false;
    }
    if let Some((_hunger, _saturation)) = item_registry.food_value(state.held_item)
        && state.food < 20
        && state.eating.is_none()
    {
        state.eating = Some(Instant::now());
        channels.anim_tx.send((state.entity_id, 3)).ok();
    }

    match state.held_item {
        261 => {
            state.bow_charging = Some(Instant::now());
            return true;
        }
        346 => {
            if !state
                .try_retract_fishing_hook(projectiles, &channels.despawn_tx)
                .await
                && let Some(uuid) = state.uuid
                && let Some(p) = player_registry.get(&uuid).await
            {
                let (dx, dy, dz) = look_direction(p.yaw, p.pitch);
                let speed = 1.5;
                let hook_eid = next_entity_id();

                let proj = Projectile {
                    entity_id: hook_eid,
                    owner_entity_id: state.entity_id,
                    kind: ProjectileKind::FishingHook,
                    x: p.x,
                    y: p.y + 1.2,
                    z: p.z, // TODO: change 1.2 to head_location
                    vx: dx * speed,
                    vy: dy * speed + 0.2,
                    vz: dz * speed,
                    ticks_alive: 0,
                };
                projectiles.write().await.push(proj.clone());
                channels
                    .projectile_spawn_tx
                    .send((
                        hook_eid,
                        state.entity_id,
                        ProjectileKind::FishingHook,
                        proj.x,
                        proj.y,
                        proj.z,
                        proj.vx,
                        proj.vy,
                        proj.vz,
                    ))
                    .ok();

                state.fishing_hook_eid = Some(hook_eid);
            }
            channels.anim_tx.send((state.entity_id, 0)).ok();
            return true;
        }
        373 => {
            let meta = state.inventory.slots[state.held_slot as usize]
                .as_ref()
                .map(|s| s.metadata)
                .unwrap_or(0);
            let is_splash = (meta & 0x4000) != 0;

            if !is_splash {
                if state.eating.is_none() {
                    state.eating = Some(Instant::now());
                    channels.anim_tx.send((state.entity_id, 3)).ok();
                }
                return true;
            }
        }
        _ => {}
    }
    false
}

/// Return true if the block interaction went well
#[allow(clippy::too_many_arguments)]
pub async fn try_with_block(
    framed: &mut Framed<TcpStream, Codec>,
    place: &PlayerBlockPlacement,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    world_blocks: &Arc<WorldBlocks>,
    world_time: &Arc<AtomicI64>,
    generator: &Arc<FlatWorldGenerator>,
    channels: &Channels,
) -> bool {
    let clicked = world_blocks.get(place.x, place.y, place.z, generator).await;
    if clicked.id == 26 {
        if !is_night(world_time.load(Relaxed)) {
            send_packet(
                framed,
                ChatMessageOut::from_json(&ChatBuilder::plain_json("You can only sleep at night")),
            )
            .await;
            return false;
        }
        state.bed_spawn = Some((place.x, place.y as i32, place.z));
        send_packet(
            framed,
            ChatBuilder::new("Respawn point set")
                .color(ChatColor::Gray)
                .into_packet(),
        )
        .await;

        state.is_sleeping = true;
        if let Some(uuid) = state.uuid {
            player_registry.update_sleeping(uuid, true).await;
        }

        send_packet(
            framed,
            UseBed {
                entity_id: state.entity_id,
                x: place.x,
                y: place.y as i32,
                z: place.z,
            },
        )
        .await;
        channels
            .bed_tx
            .send((state.entity_id, place.x, place.y as i32, place.z))
            .ok();
        return true;
    }
    false
}

fn is_night(time_of_day: i64) -> bool {
    (12542..=23459).contains(&time_of_day)
}
