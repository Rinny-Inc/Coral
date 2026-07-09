use std::{
    collections::HashMap,
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
    entity::{EntityAnimationType, UseBed},
    inventory::{ItemStack, OpenWindow, WindowItems},
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
    codec::{Codec, PlayerState, WindowType, send_packet},
};

/// Return true if the itme interaction went well
pub async fn try_with_item(
    state: &mut PlayerState,
    item_registry: &Arc<ItemRegistry>,
    player_registry: &Arc<PlayerRegistry>,
    projectiles: &Arc<RwLock<Vec<Projectile>>>,
    channels: &Channels,
) -> bool {
    if let Some((_hunger, _saturation)) = item_registry.food_value(state.held_item)
        && state.food < 20
        && state.eating.is_none()
    {
        state.eating = Some(Instant::now());
        channels
            .anim_tx
            .send((state.entity_id, EntityAnimationType::Eat))
            .ok();
    }

    match state.held_item {
        261 => {
            state.bow_charging = Some(Instant::now());
            true
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
                    left_owner: false,
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
            channels
                .anim_tx
                .send((state.entity_id, EntityAnimationType::SwingArm))
                .ok();
            true
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
                    channels
                        .anim_tx
                        .send((state.entity_id, EntityAnimationType::Eat))
                        .ok();
                }
                return true;
            }
            false
        }
        _ => false,
    }
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
    chest_storage: &Arc<RwLock<HashMap<(i32, i32, i32), Vec<Option<ItemStack>>>>>,
    channels: &Channels,
) -> bool {
    let clicked = world_blocks.get(place.x, place.y, place.z, generator).await;
    match clicked.id {
        54 | 146 => {
            if !(state.is_sneaking && place.held_item_id > 0) {
                open_chest(
                    framed,
                    state,
                    (place.x, place.y as i32, place.z),
                    chest_storage,
                )
                .await;
                return true;
            }
            false
        }
        26 => {
            if !is_night(world_time.load(Relaxed)) {
                send_packet(
                    framed,
                    ChatMessageOut::from_json(&ChatBuilder::plain_json(
                        "You can only sleep at night",
                    )),
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
            true
        }
        _ => false,
    }
}

fn is_night(time_of_day: i64) -> bool {
    (12542..=23459).contains(&time_of_day)
}

async fn open_chest(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    pos: (i32, i32, i32),
    chest_storage: &Arc<RwLock<HashMap<(i32, i32, i32), Vec<Option<ItemStack>>>>>,
) {
    state.window_id_counter = state.window_id_counter.wrapping_add(1);
    if state.window_id_counter == 0 {
        state.window_id_counter = 1;
    }
    let window_id = state.window_id_counter;

    let contents = {
        let mut storage = chest_storage.write().await;
        storage.entry(pos).or_insert_with(|| vec![None; 27]).clone()
    };

    let window_type = WindowType::Chest { window_id, pos };

    send_packet(
        framed,
        OpenWindow {
            window_id,
            window_type: window_type.clone(),
            title: ChatBuilder::new("Chest"),
            slot_count: 27,
        },
    )
    .await;

    let mut slots: Vec<(i16, u8, i16)> = Vec::with_capacity(27 + 36);

    for item in contents.iter().take(27) {
        match item {
            Some(s) => slots.push((s.item_id, s.count, s.metadata)),
            None => slots.push((-1, 0, 0)),
        }
    }

    for internal in 9..36 {
        match &state.inventory.slots[internal] {
            Some(s) => slots.push((s.item_id, s.count, s.metadata)),
            None => slots.push((-1, 0, 0)),
        }
    }
    for internal in 0..9 {
        match &state.inventory.slots[internal] {
            Some(s) => slots.push((s.item_id, s.count, s.metadata)),
            None => slots.push((-1, 0, 0)),
        }
    }

    send_packet(framed, WindowItems { window_id, slots }).await;

    state.open_window = Some(window_type);

    // TODO: play chest sound + animation
    // (0x24 BlockAction)
}
