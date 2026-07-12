use std::sync::Arc;

use coral_protocol::packets::play::chat::builder::{ChatAppender, ChatBuilder, ChatColor};
use coral_server::player::registry::PlayerRegistry;

use crate::{Command, CommandResult, make_handler};

pub fn command(player_registry: Arc<PlayerRegistry>) -> Command {
    Command {
        name: "list",
        aliases: vec![],
        description: "Show online players list",
        usage: "/list",
        handler: make_handler(move |_| {
            let registry = player_registry.clone();
            async move {
                let players = registry.get_all().await;
                let count = players.len();

                if count == 0 {
                    return CommandResult::Success(ChatBuilder::plain_json(
                        "There are 0 players online.",
                    ));
                }

                let names = players
                    .iter()
                    .map(|p| p.username.clone())
                    .collect::<Vec<String>>();

                let msg = ChatAppender::new()
                    .add(
                        ChatBuilder::new(format!("There are {} players online: ", count))
                            .color(ChatColor::Yellow),
                    )
                    .add(ChatBuilder::new(names.join(", ")).color(ChatColor::White))
                    .build();

                CommandResult::Success(msg)
            }
        }),
    }
}
