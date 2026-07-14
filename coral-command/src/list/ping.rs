use std::sync::Arc;

use coral_protocol::packets::play::chat::builder::{ChatAppender, ChatBuilder, ChatColor};
use coral_server::player::registry::PlayerRegistry;

use crate::{Command, CommandResult, make_handler};

pub fn command(player_registry: Arc<PlayerRegistry>) -> Command {
    Command {
        name: "ping",
        aliases: vec![],
        description: "Show your or another player's ping",
        usage: "/ping [player]",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            async move {
                let target_name = ctx.arg(1).unwrap_or(&ctx.sender);

                let players = registry.get_all().await;
                let Some(target) = players
                    .iter()
                    .find(|p| p.username.eq_ignore_ascii_case(target_name))
                else {
                    return CommandResult::Error(format!("Player not found: {}", target_name));
                };

                let ping = target.latency_ms;

                let color = if ping < 50 {
                    ChatColor::Green
                } else if ping < 150 {
                    ChatColor::Yellow
                } else if ping < 300 {
                    ChatColor::Gold
                } else {
                    ChatColor::Red
                };

                let label = if target.username.eq_ignore_ascii_case(&ctx.sender) {
                    "Your ping: ".to_string()
                } else {
                    format!("{}'s ping: ", target.username)
                };

                let msg = ChatAppender::new()
                    .add(ChatBuilder::new(&label).color(ChatColor::Gray))
                    .add(ChatBuilder::new(format!("{}ms", ping)).color(color).bold())
                    .build();

                CommandResult::Success(msg)
            }
        }),
    }
}
