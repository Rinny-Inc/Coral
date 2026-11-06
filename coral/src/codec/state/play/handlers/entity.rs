use coral_protocol::packets::play::{
    PlayerAbilities,
    chat::builder::ChatBuilder,
    entity::{
        EntityAction, EntityActionType, EntityAnimationType, MetadataValue, UseEntity,
        UseEntityAction,
    },
    game::{ClientStatus, ClientStatusAction, EntityStatusType, Respawn, UpdateHealth},
    movement::PlayerPositionAndLook,
};
use coral_server::{
    effects::EffectKind,
    entity_tracker::TrackedEntity,
    experience::XpOrb,
    items::armor::{apply_armor_reduction, total_defense},
    player::registry::next_entity_id,
};
use coral_types::{GameMode, dist3};
use coral_world::spawn;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use crate::codec::{
    Codec, PlayerState, send_packet,
    state::play::{PlayCtx, handlers::Flow, level_to_xp_drop, send_chunks},
};

pub(in crate::codec::state::play) fn arm_animation(state: &PlayerState, ctx: &PlayCtx<'_>) -> Flow {
    ctx.channels
        .anim_tx
        .send((state.entity_id, EntityAnimationType::SwingArm))
        .ok();
    Flow::Continue
}

pub(in crate::codec::state::play) async fn client_status(
    status: ClientStatus,
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    if let ClientStatusAction::PerformRespawn = status.action
        && state.is_dead
    {
        state.health = 20.0;
        state.food = 20;
        state.food_saturation = 5.0;
        state.is_dead = false;

        ctx.player_registry
            .update_health(&state.uuid, state.health, state.food, state.food_saturation)
            .await;

        send_packet(
            framed,
            Respawn {
                dimension: -1,
                difficulty: ctx.config.world.difficulty,
                gamemode: u8::from(state.gamemode),
                level_type: ctx.world.level_type().to_string(),
            },
        )
        .await;

        send_packet(
            framed,
            Respawn {
                dimension: 0,
                difficulty: ctx.config.world.difficulty,
                gamemode: u8::from(state.gamemode),
                level_type: ctx.world.level_type().to_string(),
            },
        )
        .await;

        let world_spawn = *ctx.spawn_point.read().await;
        let (sx, sy, sz, syaw, spitch) = spawn::respawn_position(state.bed_spawn, world_spawn);

        ctx.player_registry
            .update_position(&state.uuid, sx, sy, sz, syaw, spitch, false)
            .await;

        let (spawn_cx, spawn_cz) = spawn::spawn_chunk(sx, sz);

        state.loaded_chunks.clear();
        send_chunks(
            framed,
            ctx.client_protocol,
            ctx.world,
            ctx.tile_entities,
            ctx.server_loaded_chunks,
            spawn_cx,
            spawn_cz,
            ctx.config.server.view_distance,
            &mut state.loaded_chunks,
        )
        .await;

        (state.chunk_x, state.chunk_z) = (spawn_cx, spawn_cz);

        state.await_teleport(sx, sy, sz);
        send_packet(
            framed,
            PlayerPositionAndLook {
                x: sx,
                y: sy,
                z: sz,
                yaw: syaw,
                pitch: spitch,
                on_ground: false,
            },
        )
        .await;

        state.was_on_ground = true;

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
    Flow::Continue
}

pub(in crate::codec::state::play) async fn entity_action(
    action: EntityAction,
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    let update = match action.action {
        EntityActionType::LeaveBed => {
            state.is_sleeping = false;
            ctx.player_registry
                .update_sleeping(&state.uuid, false)
                .await;
            ctx.channels
                .anim_tx
                .send((state.entity_id, EntityAnimationType::LeaveBed))
                .ok();
            return Flow::Continue;
        }
        EntityActionType::Sneaking(start) => Some((true, start)),
        EntityActionType::Sprinting(start) => {
            // server authority dont allow sprint if food < 6
            if state.food > 6 {
                Some((false, start)) // sprinting on
            } else {
                // deny sprint
                send_packet(
                    framed,
                    PlayerAbilities {
                        flags: 0x00,
                        fly_speed: 0.05,
                        walk_speed: 0.1,
                    },
                )
                .await;
                None
            }
        }
        _ => None,
    };

    if let Some((sneaking, value)) = update {
        if sneaking {
            state.is_sneaking = value;
            ctx.player_registry
                .update_sneaking(&state.uuid, value)
                .await;
        } else {
            state.is_sprinting = value;
            ctx.player_registry
                .update_sprinting(&state.uuid, value)
                .await;
        }
        state
            .watcher
            .set(0, MetadataValue::Byte(state.entity_flags()));
    }
    Flow::Continue
}

pub(in crate::codec::state::play) async fn use_entity(
    use_entity: UseEntity,
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    match use_entity.action {
        UseEntityAction::Attack => {
            if state.gamemode == GameMode::Spectator {
                return Flow::Continue;
            }
            if let Some(target) = ctx
                .player_registry
                .get_by_entity_id(use_entity.target_entity_id)
                .await
                .as_ref()
            {
                if target.is_dead || target.no_damage_ticks > 0 {
                    return Flow::Continue;
                }
                ctx.player_registry
                    .update_no_damage_ticks(&target.uuid, 10)
                    .await;
                if let Some(me) = ctx.player_registry.get(&state.uuid).await {
                    let reach = if state.gamemode == GameMode::Creative {
                        5.0
                    } else {
                        4.0
                    };
                    if dist3(me.x, me.y, me.z, target.x, target.y, target.z) > reach {
                        return Flow::Continue;
                    }
                    let strength_bonus = state
                        .active_effects
                        .iter()
                        .find(|e| e.kind == EffectKind::Strength)
                        .map(|e| 1.3 * (e.amplifier + 1) as f32)
                        .unwrap_or(0.0);
                    let weakness_penalty = state
                        .active_effects
                        .iter()
                        .find(|e| e.kind == EffectKind::Weakness)
                        .map(|e| 0.5 * (e.amplifier + 1) as f32)
                        .unwrap_or(0.0);

                    let base_damage = ctx.item_registry.attack_damage(state.held_item.id())
                        + strength_bonus
                        - weakness_penalty;
                    let is_critical = !me.on_ground && state.fall_distance > 0.0;
                    let raw_damage = (base_damage.max(0.0)) * if is_critical { 1.5 } else { 1.0 };

                    let target_inventory_armor = ctx.player_registry.get_armor(&target.uuid).await;
                    let total_armor = total_defense(
                        ctx.item_registry,
                        target_inventory_armor.0,
                        target_inventory_armor.1,
                        target_inventory_armor.2,
                        target_inventory_armor.3,
                    );
                    let resistance = ctx
                        .player_registry
                        .get_effects(&target.uuid)
                        .await
                        .iter()
                        .find(|e| e.kind == EffectKind::Resistance)
                        .map(|e| 0.2 * (e.amplifier + 1) as f32)
                        .unwrap_or(0.0);

                    let damage =
                        apply_armor_reduction(raw_damage, total_armor) * (1.0 - resistance);
                    let new_health = (target.health - damage).max(0.0);

                    ctx.player_registry
                        .update_health(&state.uuid, new_health, state.food, state.food_saturation)
                        .await;

                    if new_health <= 0.0 {
                        ctx.channels
                            .chat_tx
                            .send(ChatBuilder::plain_json(&format!(
                                "{} was slain by {}",
                                target.username,
                                state.name.clone()
                            )))
                            .ok();

                        let xp_amount = level_to_xp_drop(target.xp_level);
                        if xp_amount > 0 {
                            let orb_eid = next_entity_id();
                            ctx.entity_tracker
                                .write()
                                .await
                                .track(TrackedEntity::experience_orb(
                                    orb_eid,
                                    target.x,
                                    target.y + 0.5,
                                    target.z,
                                    ctx.config.tracking.experience_orb,
                                ));

                            ctx.xp_orbs.write().await.push(XpOrb {
                                entity_id: orb_eid,
                                x: target.x,
                                y: target.y + 0.5,
                                z: target.z,
                                vy: 0.3,
                                amount: xp_amount,
                                ticks_alive: 0,
                            });
                            ctx.channels
                                .xp_orb_spawn_tx
                                .send((orb_eid, target.x, target.y + 0.5, target.z, xp_amount))
                                .ok();
                        }
                    }

                    ctx.channels
                        .dmg_tx
                        .send((
                            target.uuid,
                            new_health,
                            target.food,
                            target.food_saturation,
                            state.entity_id,
                        ))
                        .ok();

                    let broke = state.damage_item(2, ctx.item_registry);
                    state
                        .sync_held_slot_after_damage(
                            framed,
                            ctx.player_registry,
                            ctx.item_registry,
                            ctx.channels,
                            broke,
                        )
                        .await;

                    ctx.channels
                        .sound_tx
                        .send(("game.player.hurt", target.x, target.y, target.z, 1.0, 63))
                        .ok();
                    ctx.channels
                        .status_tx
                        .send((use_entity.target_entity_id, EntityStatusType::HurtAnimation))
                        .ok();
                    ctx.channels
                        .anim_tx
                        .send((use_entity.target_entity_id, EntityAnimationType::TakeDamage))
                        .ok();

                    if is_critical {
                        ctx.channels
                            .anim_tx
                            .send((state.entity_id, EntityAnimationType::CriticalEffect(false)))
                            .ok();
                        ctx.channels
                            .particle_tx
                            .send((
                                state.entity_id,
                                1,
                                target.x as f32,
                                target.y as f32 + 1.0,
                                target.z as f32,
                                0.3,
                                0.3,
                                0.3,
                                0.0,
                                8,
                                [0, 0],
                            ))
                            .ok();
                    }
                }
            }
        }
        _ => {}
    }
    Flow::Continue
}
