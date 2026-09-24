use std::sync::Arc;

use coral_protocol::packets::play::chat::builder::{ChatAppender, ChatBuilder};
use coral_server::{player::registry::PlayerRegistry, whitelist::WhitelistFile};
use coral_types::offline_uuid;
use tokio::sync::RwLock;

use crate::{Command, CommandResult, make_handler};

pub fn command(
    player_registry: Arc<PlayerRegistry>,
    whitelist: Arc<RwLock<WhitelistFile>>,
) -> Command {
    Command {
        name: "whitelist",
        aliases: vec!["wl"],
        description: "Manage the whitelist",
        usage: "/whitelist <add|remove|list> [player]",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let whitelist = whitelist.clone();
            async move {
                let Some(sub) = ctx.arg(1) else {
                    return CommandResult::Error(
                        "Usage: /whitelist <add|remove|list> [player]".to_string(),
                    );
                };

                match sub.to_lowercase().as_str() {
                    "list" => {
                        let names = whitelist.read().await.usernames();
                        if names.is_empty() {
                            CommandResult::Success(
                                ChatAppender::new()
                                    .add(ChatBuilder::new("There are no whitelisted players."))
                                    .build(),
                            )
                        } else {
                            CommandResult::Success(
                                ChatAppender::new()
                                    .add(ChatBuilder::new(format!(
                                        "There are {} whitelisted players: {}",
                                        names.len(),
                                        names.join(", ")
                                    )))
                                    .build(),
                            )
                        }
                    }
                    "add" => {
                        let Some(name) = ctx.arg(2) else {
                            return CommandResult::Error(
                                "Usage: /whitelist <add|remove|list> [player]".to_string(),
                            );
                        };
                        let uuid = registry
                            .get_all()
                            .await
                            .iter()
                            .find(|p| p.username.eq_ignore_ascii_case(name))
                            .map(|p| p.uuid)
                            .unwrap_or_else(|| offline_uuid(name));
                        if whitelist.read().await.is_whitelisted(uuid) {
                            return CommandResult::Success(
                                ChatAppender::new()
                                    .add(ChatBuilder::new(format!(
                                        "{} is already whitelisted",
                                        name
                                    )))
                                    .build(),
                            );
                        }
                        whitelist.write().await.add(uuid, name.to_string());
                        CommandResult::Success(
                            ChatAppender::new()
                                .add(ChatBuilder::new(format!("Added {} to the whitelist", name)))
                                .build(),
                        )
                    }
                    "remove" => {
                        let Some(name) = ctx.arg(2) else {
                            return CommandResult::Error(
                                "Usage: /whitelist <add|remove|list> [player]".to_string(),
                            );
                        };

                        if whitelist.write().await.remove_by_name(name) {
                            CommandResult::Success(
                                ChatAppender::new()
                                    .add(ChatBuilder::new(format!(
                                        "Removed {} from the whitelist",
                                        name
                                    )))
                                    .build(),
                            )
                        } else {
                            CommandResult::Success(
                                ChatAppender::new()
                                    .add(ChatBuilder::new(format!("{} is not whitelisted", name)))
                                    .build(),
                            )
                        }
                    }
                    _ => CommandResult::Error(
                        "Usage: /whitelist <add|remove|list> [player]".to_string(),
                    ),
                }
            }
        }),
    }
}
