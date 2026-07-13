use std::{path::PathBuf, sync::Arc};

use coral_server::player::registry::PlayerRegistry;
use tokio::sync::RwLock;

use crate::{Command, CommandResult, make_handler};

pub async fn command(
    player_registry: Arc<PlayerRegistry>,
    spawn_point: Arc<RwLock<(f64, f64, f64, f32, f32)>>,
    world_dir: Arc<PathBuf>,
) -> Command {
    Command {
        name: "setworldspawn",
        aliases: vec![],
        description: "Set the world spawn point",
        usage: "/setworldspawn [x y z]",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let spawn_point = spawn_point.clone();
            let world_dir = world_dir.clone();
            async move {
                if !ctx.is_op {
                    return CommandResult::Error("No permission.".to_string());
                }
                let (x, y, z, yaw, pitch) = if ctx.args.len() >= 4 {
                    let parse = |s: &str| s.parse::<f64>().ok();
                    match (
                        parse(ctx.arg(1).unwrap_or("")),
                        parse(ctx.arg(2).unwrap_or("")),
                        parse(ctx.arg(3).unwrap_or("")),
                    ) {
                        (Some(x), Some(y), Some(z)) => (x, y, z, 0.0, 0.0),
                        _ => return CommandResult::Error("Invalid coordinates".to_string()),
                    }
                } else {
                    let players = registry.get_all().await;
                    let Some(sender) = players
                        .iter()
                        .find(|p| p.username.eq_ignore_ascii_case(&ctx.sender))
                    else {
                        return CommandResult::Error(
                            "Constole must specify coordinates: /setworldspawn <x> <y> <z>"
                                .to_string(),
                        );
                    };
                    (
                        sender.x.floor(),
                        sender.y.floor(),
                        sender.z.floor(),
                        sender.yaw,
                        sender.pitch,
                    )
                };

                *spawn_point.write().await = (x, y, z, yaw, pitch);

                if let Err(e) = coral_world::level::write_spawn_point(
                    &world_dir, x as i32, y as i32, z as i32, yaw, pitch,
                )
                .await
                {
                    return CommandResult::Error(format!(
                        "Set spawn in memory but failed to save: {}",
                        e
                    ));
                }

                CommandResult::Success(format!(
                    "Set the world spawn point to ({}, {}, {}, {}, {})",
                    x as i32, y as i32, z as i32, yaw, pitch
                ))
            }
        }),
    }
}
