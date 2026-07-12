use std::sync::Arc;

use coral_protocol::packets::play::chat::builder::{ChatAppender, ChatBuilder, ChatColor};
use coral_server::player::registry::PlayerRegistry;
use coral_types::{GameMode, GamemodeUpdate};
use tokio::sync::broadcast::Sender;

use crate::{Command, CommandResult, make_handler};

// TODO: in the future check if the player has the permission
pub fn command(
    player_registry: Arc<PlayerRegistry>,
    gm_tx: Arc<Sender<GamemodeUpdate>>,
) -> Command {
    Command {
        name: "gamemode",
        aliases: vec!["gm"],
        description: "Change a player's gamemode",
        usage: "/gamemode <mode> [player]",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let tx = gm_tx.clone();
            async move {
                if !ctx.is_op {
                    return CommandResult::Error("No permission.".to_string());
                }
                let Some(mode_arg) = ctx.arg(1) else {
                    return CommandResult::Error("Usage: /gamemode <mode> [player]".to_string());
                };

                let gamemode = match mode_arg.to_lowercase().as_str() {
                    "survival" | "s" | "0" => GameMode::Survival,
                    "creative" | "c" | "1" => GameMode::Creative,
                    "adventure" | "a" | "2" => GameMode::Adventure,
                    "spectator" | "sp" | "3" => GameMode::Spectator,
                    _ => return CommandResult::Error(format!("Unknown gamemode: {}", mode_arg)),
                };

                // if player arg provided, target that player — else target sender
                let target_name = ctx.arg(2).unwrap_or(&ctx.sender);

                let players = registry.get_all().await;
                let Some(target) = players
                    .iter()
                    .find(|p| p.username.to_lowercase() == target_name.to_lowercase())
                else {
                    return CommandResult::Error(format!("Player not found: {}", target_name));
                };

                let uuid = target.uuid;
                let username = target.username.clone();

                registry.update_gamemode(uuid, gamemode).await;
                tx.send((uuid, gamemode)).ok();

                CommandResult::Success(
                    ChatAppender::new()
                        .add(ChatBuilder::new("Set ").color(ChatColor::Gray))
                        .add(ChatBuilder::new(&username).color(ChatColor::White))
                        .add(ChatBuilder::new("'s gamemode to ").color(ChatColor::Gray))
                        .add(ChatBuilder::new(format!("{:?}", gamemode)).color(ChatColor::White))
                        .build(),
                )
            }
        }),
    }
}
