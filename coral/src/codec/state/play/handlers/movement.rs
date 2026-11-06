use std::time::{Duration, Instant};

use coral_protocol::packets::play::movement::{MoveKind, MovementBroadcast, PlayerMovements};
use coral_server::player::Player;
use coral_world::spawn;
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use crate::codec::{
    Codec, PlayerState,
    state::play::{PlayCtx, handle_landing, handlers::Flow, normalize_yaw, update_chunks},
};

struct Resolved {
    x: f64,
    y: f64,
    z: f64,
    yaw: f32,
    pitch: f32,
    dx: i64,
    dy: i64,
    dz: i64,
    position_changed: bool,
}

fn resolve(mv: &PlayerMovements, p: &Player) -> Resolved {
    let (x, y, z) = mv.position.unwrap_or((p.x, p.y, p.z));
    let (yaw, pitch) = mv.rotation.unwrap_or((p.yaw, p.pitch));
    let yaw = normalize_yaw(yaw);
    let pitch = pitch.clamp(-90.0, 90.0);

    let dx = ((x - p.x) * 32.0).round() as i64;
    let dy = ((y - p.y) * 32.0).round() as i64;
    let dz = ((z - p.z) * 32.0).round() as i64;

    Resolved {
        x,
        y,
        z,
        yaw,
        pitch,
        dx,
        dy,
        dz,
        position_changed: dx != 0 || dy != 0 || dz != 0,
    }
}

async fn settle_pending_teleport(
    mv: &PlayerMovements,
    r: &Resolved,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Option<Flow> {
    let (tx, ty, tz) = state.pending_teleport?;

    let expired = state
        .pending_teleport_at
        .is_none_or(|t| t.elapsed() > Duration::from_secs(5));
    let arrived = mv.position.is_some_and(|(cx, cy, cz)| {
        (cx - tx).abs() < 1.0 && (cy - ty).abs() < 1.0 && (cz - tz).abs() < 1.0
    });

    if !arrived && !expired {
        return Some(Flow::Continue);
    }

    if expired && !arrived {
        tracing::warn!(
            "{} never confirmed a teleport to ({tx:.1}, {ty:.1}, {tz:.1}); accepting ({:.1}, {:.1}, {:.1})",
            state.name,
            r.x,
            r.y,
            r.z
        );
    }

    state.pending_teleport = None;
    state.pending_teleport_at = None;
    state.first_position_received = true;
    state.fall_distance = 0.0;
    state.velocity = (0.0, 0.0, 0.0);
    state.was_on_ground = mv.on_ground;
    state.last_position = (r.x, r.y, r.z);
    state.last_position_tick = Instant::now();
    (state.chunk_x, state.chunk_z) = spawn::spawn_chunk(r.x, r.z);

    ctx.player_registry
        .update_position(&state.uuid, r.x, r.y, r.z, r.yaw, r.pitch, mv.on_ground)
        .await;
    ctx.player_registry
        .update_velocity(&state.uuid, state.velocity)
        .await;
    ctx.entity_tracker
        .write()
        .await
        .update_position(state.entity_id, r.x, r.y, r.z);
    ctx.channels
        .pos_tx
        .send(MovementBroadcast {
            uuid: state.uuid,
            entity_id: state.entity_id,
            kind: MoveKind::Teleport {
                x: r.x,
                y: r.y,
                z: r.z,
                yaw: r.yaw,
                pitch: r.pitch,
                on_ground: mv.on_ground,
            },
            head_yaw: Some(r.yaw),
        })
        .ok();
    Some(Flow::Continue)
}

fn broadcast_kind(
    r: &Resolved,
    rotation_changed: bool,
    needs_teleport: bool,
    on_ground: bool,
) -> Option<MoveKind> {
    Some(if needs_teleport {
        MoveKind::Teleport {
            x: r.x,
            y: r.y,
            z: r.z,
            yaw: r.yaw,
            pitch: r.pitch,
            on_ground,
        }
    } else if r.position_changed && rotation_changed {
        MoveKind::LookAndRelative {
            dx: r.dx as i8,
            dy: r.dy as i8,
            dz: r.dz as i8,
            yaw: r.yaw,
            pitch: r.pitch,
            on_ground,
        }
    } else if r.position_changed {
        MoveKind::Relative {
            dx: r.dx as i8,
            dy: r.dy as i8,
            dz: r.dz as i8,
            on_ground,
        }
    } else if rotation_changed {
        MoveKind::Look {
            yaw: r.yaw,
            pitch: r.pitch,
            on_ground,
        }
    } else {
        return None;
    })
}

pub(in crate::codec::state::play) async fn player_moved(
    mv: PlayerMovements,
    p: &Player,
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    let r = resolve(&mv, p);
    let prev_velocity = state.velocity;

    if let Some(flow) = settle_pending_teleport(&mv, &r, state, ctx).await {
        return flow;
    }

    if mv.position.is_some() {
        if !mv.on_ground && r.y < p.y {
            state.fall_distance += (p.y - r.y) as f32;
        } else {
            state.fall_distance = 0.0;
        }

        if r.position_changed {
            let now = Instant::now();
            let dt = now.duration_since(state.last_position_tick).as_secs_f64();

            if dt > 0.0 {
                let raw_vx = (r.x - state.last_position.0) / dt / 20.0;
                let raw_vy = (r.y - state.last_position.1) / dt / 20.0;
                let raw_vz = (r.z - state.last_position.2) / dt / 20.0;

                let smoothing = 0.6;
                state.velocity.0 = state.velocity.0 * (1.0 - smoothing) + raw_vx * smoothing;
                state.velocity.1 = state.velocity.1 * (1.0 - smoothing) + raw_vy * smoothing;
                state.velocity.2 = state.velocity.2 * (1.0 - smoothing) + raw_vz * smoothing;
            }

            let vel_delta = (
                (state.velocity.0 - prev_velocity.0).abs(),
                (state.velocity.1 - prev_velocity.1).abs(),
                (state.velocity.2 - prev_velocity.2).abs(),
            );

            if vel_delta.0 > 0.1 || vel_delta.1 > 0.1 || vel_delta.2 > 0.1 {
                ctx.channels
                    .velocity_broadcast_tx
                    .send((
                        state.entity_id,
                        state.velocity.0,
                        state.velocity.1,
                        state.velocity.2,
                    ))
                    .ok();
            }

            state.last_position = (r.x, r.y, r.z);
            state.last_position_tick = now;

            ctx.player_registry
                .update_velocity(&state.uuid, state.velocity)
                .await;
            ctx.entity_tracker
                .write()
                .await
                .update_position(state.entity_id, r.x, r.y, r.z);
            let (new_chunk_x, new_chunk_z) = ((r.x as i32) >> 4, (r.z as i32) >> 4);

            if new_chunk_x != state.chunk_x || new_chunk_z != state.chunk_z {
                (state.chunk_x, state.chunk_z) = (new_chunk_x, new_chunk_z);
                update_chunks(
                    framed,
                    ctx.client_protocol,
                    ctx.world,
                    ctx.tile_entities,
                    ctx.server_loaded_chunks,
                    new_chunk_x,
                    new_chunk_z,
                    ctx.config.server.view_distance,
                    &mut state.loaded_chunks,
                )
                .await;
            }

            let distance = ((r.x - p.x).powi(2) + (r.z - p.z).powi(2)).sqrt();
            if state.is_sprinting {
                state.food_exhaustion += 0.1 * distance as f32;
            } else {
                state.food_exhaustion += 0.01 * distance as f32;
            }
        } else {
            let decay = 0.8;
            state.velocity.0 *= decay;
            state.velocity.1 *= decay;
            state.velocity.2 *= decay;
            if state.velocity.0.abs() < 0.001 {
                state.velocity.0 = 0.0;
            }
            if state.velocity.1.abs() < 0.001 {
                state.velocity.1 = 0.0;
            }
            if state.velocity.2.abs() < 0.001 {
                state.velocity.2 = 0.0;
            }
            ctx.player_registry
                .update_velocity(&state.uuid, state.velocity)
                .await;
        }
    }

    let rotation_changed = (r.yaw - p.yaw).abs() > 0.5 || (r.pitch - p.pitch).abs() > 0.5;
    let needs_teleport = r.dx.abs() > 127 || r.dy.abs() > 127 || r.dz.abs() > 127;

    if r.position_changed || rotation_changed {
        handle_landing(
            framed,
            state,
            ctx.player_registry,
            &ctx.channels.chat_tx,
            &ctx.channels.sound_tx,
            r.x,
            r.y,
            r.z,
            mv.on_ground,
        )
        .await;
        ctx.player_registry
            .update_position(&state.uuid, r.x, r.y, r.z, r.yaw, r.pitch, mv.on_ground)
            .await;
    }

    let Some(kind) = broadcast_kind(&r, rotation_changed, needs_teleport, mv.on_ground) else {
        return Flow::Continue;
    };

    ctx.channels
        .pos_tx
        .send(MovementBroadcast {
            uuid: state.uuid,
            entity_id: state.entity_id,
            kind,
            head_yaw: if rotation_changed { Some(r.yaw) } else { None },
        })
        .ok();
    Flow::Continue
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f64, y: f64, z: f64, yaw: f32, pitch: f32) -> Player {
        Player::new(
            1,
            uuid::Uuid::nil(),
            "tester".to_string(),
            vec![],
            x,
            y,
            z,
            yaw,
            pitch,
            coral_types::GameMode::Survival,
            20.0,
            20,
            5.0,
            0,
            0,
        )
    }

    fn kind_name(kind: &Option<MoveKind>) -> &'static str {
        match kind {
            None => "none",
            Some(MoveKind::Teleport { .. }) => "Teleport",
            Some(MoveKind::Relative { .. }) => "Relative",
            Some(MoveKind::Look { .. }) => "Look",
            Some(MoveKind::LookAndRelative { .. }) => "LookAndRelative",
        }
    }

    fn mv(position: Option<(f64, f64, f64)>, rotation: Option<(f32, f32)>) -> PlayerMovements {
        PlayerMovements {
            position,
            rotation,
            on_ground: true,
        }
    }

    #[test]
    fn a_look_packet_keeps_the_last_known_position() {
        let p = at(10.0, 64.0, -5.0, 0.0, 0.0);
        let r = resolve(&mv(None, Some((90.0, 45.0))), &p);

        assert_eq!((r.x, r.y, r.z), (10.0, 64.0, -5.0));
        assert_eq!((r.yaw, r.pitch), (90.0, 45.0));
        assert!(
            !r.position_changed,
            "a look packet does not move the player"
        );
    }

    #[test]
    fn a_position_packet_keeps_the_last_known_rotation() {
        let p = at(0.0, 64.0, 0.0, 135.0, -20.0);
        let r = resolve(&mv(Some((1.0, 64.0, 0.0)), None), &p);

        assert_eq!((r.yaw, r.pitch), (135.0, -20.0));
        assert!(r.position_changed);
    }

    #[test]
    fn deltas_are_in_thirty_seconds_of_a_block() {
        let p = at(0.0, 64.0, 0.0, 0.0, 0.0);
        let r = resolve(&mv(Some((1.0, 64.5, -2.0)), None), &p);

        assert_eq!(r.dx, 32, "one block is 32 units");
        assert_eq!(r.dy, 16);
        assert_eq!(r.dz, -64);
    }

    #[test]
    fn a_move_too_small_to_express_is_not_a_move() {
        let p = at(0.0, 64.0, 0.0, 0.0, 0.0);
        let r = resolve(&mv(Some((0.001, 64.0, 0.0)), None), &p);

        assert_eq!((r.dx, r.dy, r.dz), (0, 0, 0));
        assert!(!r.position_changed);
    }

    #[test]
    fn pitch_is_clamped_and_yaw_is_normalised() {
        let p = at(0.0, 64.0, 0.0, 0.0, 0.0);

        let r = resolve(&mv(None, Some((0.0, 500.0))), &p);
        assert_eq!(r.pitch, 90.0, "pitch cannot exceed straight down");

        let r = resolve(&mv(None, Some((0.0, -500.0))), &p);
        assert_eq!(r.pitch, -90.0);

        let r = resolve(&mv(None, Some((730.0, 0.0))), &p);
        assert!(
            (-180.0..=360.0).contains(&r.yaw),
            "yaw {} was not normalised",
            r.yaw
        );
    }

    fn resolved(dx: i64, dy: i64, dz: i64) -> Resolved {
        Resolved {
            x: 1.0,
            y: 64.0,
            z: 2.0,
            yaw: 90.0,
            pitch: 0.0,
            dx,
            dy,
            dz,
            position_changed: dx != 0 || dy != 0 || dz != 0,
        }
    }

    #[test]
    fn nothing_moving_broadcasts_nothing() {
        assert!(broadcast_kind(&resolved(0, 0, 0), false, false, true).is_none());
    }

    #[test]
    fn each_combination_picks_its_own_broadcast() {
        assert!(matches!(
            broadcast_kind(&resolved(0, 0, 0), true, false, true),
            Some(MoveKind::Look { .. })
        ));
        assert!(matches!(
            broadcast_kind(&resolved(4, 0, 0), false, false, true),
            Some(MoveKind::Relative { .. })
        ));
        assert!(matches!(
            broadcast_kind(&resolved(4, 0, 0), true, false, true),
            Some(MoveKind::LookAndRelative { .. })
        ));
        assert!(matches!(
            broadcast_kind(&resolved(4, 0, 0), true, true, true),
            Some(MoveKind::Teleport { .. })
        ));
    }

    #[test]
    fn a_delta_past_an_i8_would_wrap_if_it_were_not_teleported() {
        let far = resolved(200, 0, 0);

        match broadcast_kind(&far, false, true, true) {
            Some(MoveKind::Teleport { .. }) => {}
            ref other => panic!("a 200-unit move must teleport, got {}", kind_name(other)),
        }

        match broadcast_kind(&far, false, false, true) {
            Some(MoveKind::Relative { dx, .. }) => assert_eq!(
                dx, -56,
                "200 as i8 wraps to -56: the move would go backwards"
            ),
            ref other => panic!("expected Relative, got {}", kind_name(other)),
        }
    }

    #[test]
    fn the_largest_delta_a_relative_move_can_carry_is_127() {
        match broadcast_kind(&resolved(127, -128, 0), false, false, true) {
            Some(MoveKind::Relative { dx, dy, .. }) => {
                assert_eq!(dx, 127);
                assert_eq!(dy, -128);
            }
            ref other => panic!("expected Relative, got {}", kind_name(other)),
        }
    }
}
