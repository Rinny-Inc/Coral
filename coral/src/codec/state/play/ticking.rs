use std::{collections::HashMap, sync::Arc, time::Instant};

use coral_config::Config;
use coral_protocol::packets::play::{
    chat::builder::ChatBuilder, game::UpdateHealth, inventory::SetSlot,
};
use coral_server::{
    bounding_box::EntityBounds, effects::EffectKind, items::ItemRegistry, registry::PlayerRegistry,
};
use coral_types::GameMode;
use rand::RngExt;
use tokio::{
    net::TcpStream,
    sync::{RwLock, broadcast::Sender},
};
use tokio_util::codec::Framed;

use crate::{
    BreakAnimation, Channels, EquipmentUpdate, ItemInfo, ItemPickup, SoundEffect,
    codec::{
        Codec, PlayerState, send_packet,
        state::play::{apply_potion_effect, remove_effect, send_held_equip},
    },
};

#[allow(clippy::too_many_arguments)]
pub async fn handle_tick(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    item_registry: &Arc<ItemRegistry>,
    config: &Config,
    item_spawn_times: &Arc<RwLock<HashMap<i32, Instant>>>,
    item_positions: &Arc<RwLock<HashMap<i32, ItemInfo>>>,
    channels: &Channels,
) {
    tick_eating(
        framed,
        state,
        player_registry,
        item_registry,
        &channels.equip_tx,
        &channels.sound_tx,
    )
    .await;
    tick_item_pickup(
        framed,
        state,
        player_registry,
        item_positions,
        item_spawn_times,
        &channels.pickup_tx,
        &channels.equip_tx,
        &channels.sound_tx,
    )
    .await;
    tick_block_breaking_progress(state, &channels.break_tx).await;
    tick_food_and_regen(framed, state, config, player_registry, &channels.chat_tx).await;
    tick_effects(framed, state, player_registry, &channels.chat_tx).await;
    tick_void_damage(framed, state, player_registry, &channels.chat_tx).await;
}

async fn tick_block_breaking_progress(
    state: &mut PlayerState,
    break_tx: &Arc<Sender<BreakAnimation>>,
) {
    if let Some((bx, by, bz)) = state.breaking_block
        && state.breaking_required_ticks > 0
    {
        let elapsed = (state.tick_count - state.breaking_started_tick).max(0) as u32;
        let progress = elapsed as f32 / state.breaking_required_ticks as f32;
        let stage = (progress * 10.0).floor() as u8;
        let stage = stage.min(9);
        break_tx.send((state.entity_id, bx, by, bz, stage)).ok();
    }
}
async fn tick_void_damage(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    chat_tx: &Arc<Sender<String>>,
) {
    if state.tick_count % 10 == 0
        && let Some(uuid) = state.uuid
        && let Some(p) = player_registry.get(&uuid).await
        && p.y < -64.0
    {
        let died = state.damage_player(framed, 4.0, player_registry).await;
        if died && let Some(ref name) = state.name {
            chat_tx
                .send(ChatBuilder::plain_json(&format!(
                    "{} fell out of the world",
                    name
                )))
                .ok();
        }
    }
}
#[allow(clippy::too_many_arguments)]
async fn tick_item_pickup(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    item_positions: &Arc<RwLock<HashMap<i32, ItemInfo>>>,
    item_spawn_times: &Arc<RwLock<HashMap<i32, Instant>>>,
    pickup_tx: &Arc<Sender<ItemPickup>>,
    equip_tx: &Arc<Sender<EquipmentUpdate>>,
    sound_tx: &Arc<Sender<SoundEffect>>,
) {
    if let Some(uuid) = state.uuid
        && let Some(p) = player_registry.get(&uuid).await
    {
        let mut items = item_positions.write().await;
        let mut picked_up = vec![];

        for (eid, (_item_eid, ix, iy, iz, item_id, count, metadata)) in items.iter() {
            let player_bb = EntityBounds::player(state.is_sneaking);

            let item_bb = EntityBounds::item();

            if player_bb.intersects(p.x, p.y, p.z, &item_bb, *ix, *iy, *iz) {
                let age = {
                    let spawn_time = item_spawn_times.read().await;
                    spawn_time
                        .get(eid)
                        .map(|t| t.elapsed().as_secs_f32())
                        .unwrap_or(0.0)
                };

                if age < 0.5 {
                    continue;
                }

                let slot_index = state
                    .inventory
                    .add_item_get_slot(*item_id, *count, *metadata);
                if let Some((packet_slot, internal_idx)) = slot_index {
                    picked_up.push(*eid);

                    let actual_count = state.inventory.slots[internal_idx]
                        .as_ref()
                        .map(|s| s.count)
                        .unwrap_or(*count);

                    send_packet(
                        framed,
                        SetSlot {
                            window_id: 0,
                            slot: packet_slot,
                            item_id: *item_id,
                            count: actual_count,
                            metadata: *metadata,
                        },
                    )
                    .await;

                    if internal_idx == state.held_slot as usize {
                        state.held_item = *item_id;
                        if let Some(uuid) = state.uuid {
                            player_registry.update_held_item(uuid, *item_id).await;
                        }
                        send_held_equip(equip_tx, state);
                    }
                }
            }
        }
        for eid in picked_up {
            items.remove(&eid);
            item_spawn_times.write().await.remove(&eid);
            pickup_tx.send((state.entity_id, uuid, eid)).ok();
            let pitch = 63 + (rand::rng().random_range(-12i8..=12) as i16) as u8;
            sound_tx
                .send(("random.pop".to_string(), p.x, p.y, p.z, 0.2, pitch))
                .ok();
        }
    }
}
async fn tick_eating(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    item_registry: &Arc<ItemRegistry>,
    equip_tx: &Arc<Sender<EquipmentUpdate>>,
    sound_tx: &Arc<Sender<SoundEffect>>,
) {
    if let Some(started) = state.eating
        && started.elapsed().as_millis() >= 1600
    {
        state.eating = None;

        if state.held_item == 373 {
            let meta = state.inventory.slots[state.held_slot as usize]
                .as_ref()
                .map(|s| s.metadata)
                .unwrap_or(0);
            let effects = coral_server::items::potions::potion_effects(meta);
            for pe in effects {
                apply_potion_effect(framed, state, player_registry, pe).await;
            }
            let hotbar_slot = state.held_slot as usize;
            state.inventory.slots[hotbar_slot] = None;
            let packet_slot = (36 + hotbar_slot) as i16;
            send_packet(
                framed,
                SetSlot {
                    window_id: 0,
                    slot: packet_slot,
                    item_id: -1,
                    count: 0,
                    metadata: 0,
                },
            )
            .await;
            state.held_item = -1;
            if let Some(uuid) = state.uuid {
                player_registry.update_held_item(uuid, -1).await;
            }
            send_held_equip(equip_tx, state);
        } else if let Some((hunger, saturation)) = item_registry.food_value(state.held_item) {
            state.food = (state.food + hunger).min(20);
            state.food_saturation = (state.food_saturation + saturation).min(state.food as f32);

            let hotbar_slot = state.held_slot as usize;
            if let Some(slot) = state.inventory.slots[hotbar_slot].as_mut() {
                slot.count -= 1;
                let remaining = if slot.count == 0 {
                    state.inventory.slots[hotbar_slot] = None;
                    None
                } else {
                    Some((slot.item_id, slot.count, slot.metadata))
                };

                let packed_slot = (36 + hotbar_slot) as i16;
                send_packet(
                    framed,
                    SetSlot {
                        window_id: 0,
                        slot: packed_slot,
                        item_id: remaining.map(|(id, _, _)| id).unwrap_or(-1),
                        count: remaining.map(|(_, c, _)| c).unwrap_or(0),
                        metadata: remaining.map(|(_, _, m)| m).unwrap_or(0),
                    },
                )
                .await;

                state.held_item = remaining.map(|(id, _, _)| id).unwrap_or(-1);
                if let Some(uuid) = state.uuid {
                    player_registry
                        .update_held_item(uuid, state.held_item)
                        .await;
                }
                send_held_equip(equip_tx, state);
            }
            if let Some(uuid) = state.uuid {
                player_registry
                    .update_health(uuid, state.health, state.food, state.food_saturation)
                    .await;
            }
            send_packet(
                framed,
                UpdateHealth {
                    health: state.health,
                    food: state.food,
                    food_saturation: state.food_saturation,
                },
            )
            .await;

            if let Some(uuid) = state.uuid
                && let Some(player) = player_registry.get(&uuid).await
            {
                sound_tx
                    .send((
                        "random.burp".to_string(),
                        player.x,
                        player.y,
                        player.z,
                        0.5,
                        63,
                    ))
                    .ok();
            }
        }
    }
}
async fn tick_food_and_regen(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    config: &Config,
    player_registry: &Arc<PlayerRegistry>,
    chat_tx: &Arc<Sender<String>>,
) {
    if state.gamemode != GameMode::Survival {
        return;
    }
    if config.world.difficulty == 0 {
        if state.health < 20.0 {
            state.regen_timer += 1;
            if state.regen_timer >= 80 {
                state.regen_timer = 0;
                state.health = (state.health + 1.0).min(20.0);
                if let Some(uuid) = state.uuid {
                    player_registry
                        .update_health(uuid, state.health, state.food, state.food_saturation)
                        .await;
                }
                send_packet(
                    framed,
                    UpdateHealth {
                        health: state.health,
                        food: state.food,
                        food_saturation: state.food_saturation,
                    },
                )
                .await;
            }
        } else {
            state.regen_timer = 0;
        }
    } else {
        if state.food_exhaustion >= 4.0 {
            state.food_exhaustion -= 4.0;

            if state.food_saturation > 0.0 {
                state.food_saturation = (state.food_saturation - 1.0).max(0.0);
            } else if state.food > 0 {
                state.food -= 1;
                state.food_saturation = 0.0;

                if let Some(uuid) = state.uuid {
                    player_registry
                        .update_health(uuid, state.health, state.food, state.food_saturation)
                        .await;
                }

                send_packet(
                    framed,
                    UpdateHealth {
                        health: state.health,
                        food: state.food,
                        food_saturation: state.food_saturation,
                    },
                )
                .await;
            }
        }
        if state.food >= 18 && state.health < 20.0 {
            state.regen_timer += 1;

            if state.regen_timer >= 80 {
                state.regen_timer = 0;
                state.health = (state.health + 1.0).min(20.0);
                state.food_exhaustion += 3.0;

                if let Some(uuid) = state.uuid {
                    player_registry
                        .update_health(uuid, state.health, state.food, state.food_saturation)
                        .await;
                }
                send_packet(
                    framed,
                    UpdateHealth {
                        health: state.health,
                        food: state.food,
                        food_saturation: state.food_saturation,
                    },
                )
                .await;
            }
        } else {
            state.regen_timer = 0;
        }

        if state.food == 0 && state.tick_count % 80 == 0 {
            let min_health = match config.world.difficulty {
                1 => 10.0,
                2 => 1.0,
                3 => 0.0,
                _ => 1.0,
            };

            if state.health > min_health {
                state.health = (state.health - 1.0).max(min_health);
                let just_died = state.health <= 0.0 && !state.is_dead;
                state.is_dead = state.health <= 0.0;

                if just_died && let Some(ref name) = state.name {
                    chat_tx
                        .send(ChatBuilder::plain_json(&format!(
                            "{} starved to death",
                            name
                        )))
                        .ok();
                }

                if let Some(uuid) = state.uuid {
                    player_registry
                        .update_health(uuid, state.health, state.food, state.food_saturation)
                        .await;
                }

                send_packet(
                    framed,
                    UpdateHealth {
                        health: state.health,
                        food: state.food,
                        food_saturation: state.food_saturation,
                    },
                )
                .await;
            }
        }
    }
}
async fn tick_effects(
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    player_registry: &Arc<PlayerRegistry>,
    chat_tx: &Arc<Sender<String>>,
) {
    let mut expired: Vec<EffectKind> = vec![];
    let mut regen_from_effect = false;
    let mut poison_tick = false;
    let mut wither_tick = false;

    for effect in state.active_effects.iter_mut() {
        effect.remaining_ticks -= 1;
        if effect.remaining_ticks <= 0 {
            expired.push(effect.kind.clone());
            continue;
        }

        match &effect.kind {
            EffectKind::Regeneration => {
                let interval = (50 / (1 + effect.amplifier as i32)).max(1);
                if state.tick_count % interval as i64 == 0 {
                    regen_from_effect = true;
                }
            }
            EffectKind::Poison => {
                let interval = (25 / (1 + effect.amplifier as i32)).max(1);
                if state.tick_count % interval as i64 == 0 {
                    poison_tick = true;
                }
            }
            EffectKind::Wither => {
                let interval = (40 / (1 + effect.amplifier as i32)).max(1);
                if state.tick_count % interval as i64 == 0 {
                    wither_tick = true;
                }
            }
            _ => {}
        }
    }

    if regen_from_effect && state.health < 20.0 + state.absorption_hp {
        state.health = (state.health + 1.0).min(20.0 + state.absorption_hp);
        if let Some(uuid) = state.uuid {
            player_registry
                .update_health(uuid, state.health, state.food, state.food_saturation)
                .await;
        }
        send_packet(
            framed,
            UpdateHealth {
                health: state.health,
                food: state.food,
                food_saturation: state.food_saturation,
            },
        )
        .await;
    }
    if poison_tick && state.health > 1.0 {
        state.health = (state.health - 1.0).max(1.0);
        if let Some(uuid) = state.uuid {
            player_registry
                .update_health(uuid, state.health, state.food, state.food_saturation)
                .await;
        }
        send_packet(
            framed,
            UpdateHealth {
                health: state.health,
                food: state.food,
                food_saturation: state.food_saturation,
            },
        )
        .await;
    }
    if wither_tick {
        let died = state.damage_player(framed, 1.0, player_registry).await;
        if died && let Some(ref name) = state.name {
            chat_tx
                .send(ChatBuilder::plain_json(&format!("{} withered away", name)))
                .ok();
        }
    }

    for kind in expired {
        remove_effect(framed, state, kind.clone()).await;
        if kind == EffectKind::Absorption {
            state.absorption_hp = 0.0;
        }
    }
    if let Some(uuid) = state.uuid {
        player_registry
            .update_effects(uuid, state.active_effects.clone())
            .await;
    }
}
