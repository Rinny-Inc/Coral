use std::sync::Arc;

use coral_server::{ops::OpsFile, player::registry::PlayerRegistry};
use tokio::sync::RwLock;

use crate::{Command, CommandResult, make_handler};

pub fn command(player_registry: Arc<PlayerRegistry>, ops: Arc<RwLock<OpsFile>>) -> Command {
    Command {
        name: "deop",
        aliases: vec![],
        description: "Revoke operator status",
        usage: "/deop <player>",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let ops = ops.clone();
            async move {
                if !ctx.is_op {
                    return CommandResult::Error("No permission.".to_string());
                }
                let Some(target_name) = ctx.arg(1) else {
                    return CommandResult::Error("Usage: /deop <player>".to_string());
                };

                let players = registry.get_all().await;
                let Some(target) = players
                    .iter()
                    .find(|p| p.username.to_lowercase() == target_name.to_lowercase())
                else {
                    return CommandResult::Error(format!("Player not found: {}", target_name));
                };

                ops.write().await.remove(&target.uuid);

                CommandResult::Success(format!(
                    "Made {} no longer a server operator",
                    target.username
                ))
            }
        }),
    }
}
