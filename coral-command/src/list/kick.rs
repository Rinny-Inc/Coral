use std::sync::Arc;

use coral_server::player::registry::PlayerRegistry;
use coral_types::KickRequest;
use tokio::sync::broadcast::Sender;

use crate::{Command, CommandResult, make_handler};

pub fn command(
    player_registry: Arc<PlayerRegistry>,
    kick_rq_tx: Arc<Sender<KickRequest>>,
) -> Command {
    Command {
        name: "kick",
        aliases: vec![],
        description: "Kick a player from the server",
        usage: "/kick <player> [reason]",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let tx = kick_rq_tx.clone();
            async move {
                let Some(target_name) = ctx.arg(1) else {
                    return CommandResult::Error("Usage: /kick <player> [reason]".to_string());
                };

                let players = registry.get_all().await;
                let Some(target) = players
                    .iter()
                    .find(|p| p.username.eq_ignore_ascii_case(target_name))
                else {
                    return CommandResult::Error(format!("Player not found: {}", target_name));
                };

                let reason = if ctx.args.len() > 2 {
                    ctx.args_from(2)
                } else {
                    "Kick by an operator".to_string()
                };

                tx.send((target.uuid, reason.clone())).ok();
                CommandResult::Success(format!("Kicked {}: {}", target.username, reason))
            }
        }),
    }
}
