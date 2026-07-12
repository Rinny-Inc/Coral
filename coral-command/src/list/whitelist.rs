use std::sync::Arc;

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
                            CommandResult::Success("There are no whitelisted players.".to_string())
                        } else {
                            CommandResult::Success(format!(
                                "There are {} whitelisted players: {}",
                                names.len(),
                                names.join(", ")
                            ))
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
                        whitelist.write().await.add(uuid, name.to_string());
                        CommandResult::Success(format!("Added {} to the whitelist", name))
                    }
                    "remove" => {
                        let Some(name) = ctx.arg(2) else {
                            return CommandResult::Error(
                                "Usage: /whitelist <add|remove|list> [player]".to_string(),
                            );
                        };

                        if whitelist.write().await.remove_by_name(name) {
                            CommandResult::Success(format!("Removed {} from the whitelist", name))
                        } else {
                            CommandResult::Error(format!("{} is not whitelisted", name))
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
