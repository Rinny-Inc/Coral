use std::sync::Arc;

use coral_server::player::registry::PlayerRegistry;
use coral_types::TeleportRequest;
use tokio::sync::broadcast::Sender;

use crate::{Command, CommandResult, make_handler};

pub fn command(
    player_registry: Arc<PlayerRegistry>,
    teleport_tx: Arc<Sender<TeleportRequest>>,
) -> Command {
    Command {
        name: "tp",
        aliases: vec!["teleport"],
        description: "Teleport a player",
        usage: "/tp <player> | /tp <player> <target> | /tp <x> <y> <z>",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let tx = teleport_tx.clone();
            async move {
                let players = registry.get_all().await;

                let find = |name: &str| {
                    players
                        .iter()
                        .find(|p| p.username.eq_ignore_ascii_case(name))
                        .cloned()
                };

                match ctx.args.len() {
                    // /tp <x> <y> <z>  → teleport self to coords
                    // /tp <player>     → teleport self to that player
                    2 => {
                        let arg = ctx.arg(1).unwrap();
                        // is it a player name?
                        if let Some(target) = find(arg) {
                            let Some(sender) = find(&ctx.sender) else {
                                return CommandResult::Error(
                                    "Console must specify a player to teleport".to_string(),
                                );
                            };
                            tx.send((sender.uuid, target.x, target.y, target.z)).ok();
                            CommandResult::Success(format!("Teleported you to {}", target.username))
                        } else {
                            CommandResult::Error(format!("Player not found: {}", arg))
                        }
                    }
                    // /tp <x> <y> <z>  → self to coords
                    4 => {
                        let parse = |s: &str| s.parse::<f64>().ok();
                        match (
                            parse(ctx.arg(1).unwrap()),
                            parse(ctx.arg(2).unwrap()),
                            parse(ctx.arg(3).unwrap()),
                        ) {
                            (Some(x), Some(y), Some(z)) => {
                                let Some(sender) = find(&ctx.sender) else {
                                    return CommandResult::Error(
                                        "Console can't teleport to coordinates without a player"
                                            .to_string(),
                                    );
                                };
                                tx.send((sender.uuid, x, y, z)).ok();
                                CommandResult::Success(format!(
                                    "Teleported you to {} {} {}",
                                    x as i32, y as i32, z as i32
                                ))
                            }
                            // /tp <player> <target>  → player to target
                            _ => {
                                let (Some(moved), Some(dest)) =
                                    (find(ctx.arg(1).unwrap()), find(ctx.arg(2).unwrap()))
                                else {
                                    return CommandResult::Error("Invalid arguments. Use /tp <player> <target> or /tp <x> <y> <z>".to_string());
                                };
                                tx.send((moved.uuid, dest.x, dest.y, dest.z)).ok();
                                CommandResult::Success(format!(
                                    "Teleported {} to {}",
                                    moved.username, dest.username
                                ))
                            }
                        }
                    }
                    // /tp <player> <target> handled above when arg2 isn't a number;
                    // 3 args = /tp <player> <target>
                    3 => {
                        let (Some(moved), Some(dest)) =
                            (find(ctx.arg(1).unwrap()), find(ctx.arg(2).unwrap()))
                        else {
                            return CommandResult::Error("Player not found".to_string());
                        };
                        tx.send((moved.uuid, dest.x, dest.y, dest.z)).ok();
                        CommandResult::Success(format!(
                            "Teleported {} to {}",
                            moved.username, dest.username
                        ))
                    }
                    _ => CommandResult::Error(
                        "Usage: /tp <player> | /tp <player> <target> | /tp <x> <y> <z>".to_string(),
                    ),
                }
            }
        }),
    }
}
