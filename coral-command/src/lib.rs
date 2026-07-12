use std::{collections::HashMap, pin::Pin, sync::Arc, vec};

use coral_protocol::packets::play::chat::builder::{ChatAppender, ChatBuilder, ChatColor};
use coral_server::{ops::OpsFile, player::registry::PlayerRegistry, whitelist::WhitelistFile};
use coral_types::{DamageEvent, GameMode, GamemodeUpdate, PrivateMessage};
use tokio::sync::{RwLock, broadcast::Sender};
use uuid::Uuid;

pub mod list;

pub type CommandFuture = Pin<Box<dyn Future<Output = CommandResult> + Send>>;
pub type CommandHandler = Arc<dyn Fn(CommandContext) -> CommandFuture + Send + Sync>;

#[derive(Debug, Clone)]
pub struct CommandContext {
    pub sender: String,
    pub args: Vec<String>,
    pub reply_target: Option<String>,
    pub is_op: bool,
}
impl CommandContext {
    pub fn arg(&self, index: usize) -> Option<&str> {
        self.args.get(index).map(|s| s.as_str())
    }
    pub fn args_from(&self, index: usize) -> String {
        self.args[index..].join(" ")
    }
}

#[derive(Debug)]
pub enum CommandResult {
    Success(String),
    Broadcast(String),
    Error(String),
    None,
}

pub struct Command {
    pub name: &'static str,
    pub aliases: Vec<&'static str>,
    pub description: &'static str,
    pub usage: &'static str,
    pub handler: CommandHandler,
}

pub struct CommandDispatcher {
    commands: RwLock<HashMap<String, Command>>,
}
impl CommandDispatcher {
    pub fn new() -> Self {
        Self {
            commands: RwLock::new(HashMap::with_capacity(1)),
        }
    }

    pub async fn register(&self, command: Command) {
        let mut commands = self.commands.write().await;

        for alias in &command.aliases {
            commands.insert(
                alias.to_string(),
                Command {
                    name: command.name,
                    aliases: vec![],
                    description: command.description,
                    usage: command.usage,
                    handler: command.handler.clone(),
                },
            );
        }

        commands.insert(command.name.to_string(), command);
    }

    pub async fn dispatch(&self, ctx: CommandContext) -> CommandResult {
        let name = ctx.args[0].to_lowercase();
        let commands = self.commands.read().await;
        match commands.get(&name) {
            Some(cmd) => (cmd.handler)(ctx).await,
            None => CommandResult::Error("Unknown command!".to_string()),
        }
    }

    pub async fn completions(&self, partial: &str) -> Vec<String> {
        self.commands
            .read()
            .await
            .keys()
            .filter(|k| k.starts_with(partial))
            .map(|k| format!("/{}", k))
            .collect()
    }
}

pub fn make_handler<F, Fut>(f: F) -> CommandHandler
where
    F: Fn(CommandContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = CommandResult> + Send + 'static,
{
    Arc::new(move |ctx| Box::pin(f(ctx)))
}

pub fn list_command(player_registry: Arc<PlayerRegistry>) -> Command {
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

// TODO: in the future check if the player has the permission
pub fn gamemode_command(
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
pub fn kill_command(
    player_registry: Arc<PlayerRegistry>,
    dmg_tx: Arc<Sender<DamageEvent>>,
) -> Command {
    Command {
        name: "kill",
        aliases: vec![],
        description: "Kill yourself or another player",
        usage: "/kill [player]",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let tx = dmg_tx.clone();
            async move {
                if !ctx.is_op {
                    return CommandResult::Error("No permission.".to_string());
                }
                let target_name = ctx.arg(1).unwrap_or(&ctx.sender);
                let players = registry.get_all().await;
                let Some(target) = players
                    .iter()
                    .find(|p| p.username.to_lowercase() == target_name.to_lowercase())
                else {
                    return CommandResult::Error(format!("Player not found: {}", target_name));
                };

                registry
                    .update_health(target.uuid, 0.0, target.food, target.food_saturation)
                    .await;
                tx.send((
                    target.uuid,
                    0.0,
                    target.food,
                    target.food_saturation,
                    target.entity_id,
                ))
                .ok();

                CommandResult::Broadcast(format!("{} was killed", target.username))
            }
        }),
    }
}

pub fn op_command(player_registry: Arc<PlayerRegistry>, ops: Arc<RwLock<OpsFile>>) -> Command {
    Command {
        name: "op",
        aliases: vec![],
        description: "Grant operator status",
        usage: "/op <player>",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let ops = ops.clone();
            async move {
                if !ctx.is_op {
                    return CommandResult::Error("No permission.".to_string());
                }
                let Some(target_name) = ctx.arg(1) else {
                    return CommandResult::Error("Usage: /op <player>".to_string());
                };

                let players = registry.get_all().await;
                let Some(target) = players
                    .iter()
                    .find(|p| p.username.to_lowercase() == target_name.to_lowercase())
                else {
                    return CommandResult::Error(format!("Player not found: {}", target_name));
                };

                ops.write().await.add(target.uuid, &target.username, 4);

                CommandResult::Success(format!("Made {} a server operator", target.username))
            }
        }),
    }
}
pub fn deop_command(player_registry: Arc<PlayerRegistry>, ops: Arc<RwLock<OpsFile>>) -> Command {
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

pub fn whitelist_command(
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
                            .unwrap_or_else(|| {
                                Uuid::new_v3(
                                    &Uuid::NAMESPACE_DNS,
                                    format!("OfflinePlayer:{}", name).as_bytes(),
                                )
                            });
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

pub fn say_command() -> Command {
    Command {
        name: "say",
        aliases: vec![],
        description: "Broadcast a message to all players",
        usage: "/say <message>",
        handler: make_handler(move |ctx| async move {
            if ctx.args.len() < 2 {
                return CommandResult::Error("Usage: /say <message>".to_string());
            }
            let message = ctx.args_from(1);

            /*let json = ChatAppender::new()
            .add(ChatBuilder::new(format!("[{}]", ctx.sender)).color(ChatColor::LightPurple))
            .add(ChatBuilder::new(message).color(ChatColor::White))
            .build();*/

            let format = format!("[{}] {}", ctx.sender, message);

            CommandResult::Broadcast(format)
        }),
    }
}

pub fn msg_command(
    player_registry: Arc<PlayerRegistry>,
    private_msg_tx: Arc<Sender<PrivateMessage>>,
) -> Command {
    Command {
        name: "msg",
        aliases: vec!["tell", "w", "whisper"],
        description: "Send a private message",
        usage: "/msg <player> <message>",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let tx = private_msg_tx.clone();
            async move {
                let Some(target) = ctx.arg(1) else {
                    return CommandResult::Error("Usage: /msg <player> <message>".to_string());
                };
                if ctx.args.len() < 3 {
                    return CommandResult::Error("Usage: /msg <player> <message>".to_string());
                }
                let message = ctx.args_from(2);

                let online = registry.get_all().await;
                let Some(target_player) = online
                    .iter()
                    .find(|p| p.username.eq_ignore_ascii_case(target))
                else {
                    return CommandResult::Error(format!("Player not found: {}", target));
                };

                if target_player.username.eq_ignore_ascii_case(&ctx.sender) {
                    return CommandResult::Error("You can't message yourself".to_string());
                }
                tx.send((
                    ctx.sender.clone(),
                    target_player.username.clone(),
                    message.clone(),
                ))
                .ok();

                let echo = ChatAppender::new()
                    .add(
                        ChatBuilder::new(format!("You -> {}: ", target_player.username))
                            .color(ChatColor::Gray)
                            .italic(),
                    )
                    .add(ChatBuilder::new(&message).color(ChatColor::Gray).italic())
                    .build();
                CommandResult::Success(echo)
            }
        }),
    }
}
pub fn reply_command(
    player_registry: Arc<PlayerRegistry>,
    private_msg_tx: Arc<Sender<(String, String, String)>>,
) -> Command {
    Command {
        name: "reply",
        aliases: vec!["r"],
        description: "Reply to the last player who messaged you",
        usage: "/reply <message>",
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let tx = private_msg_tx.clone();
            async move {
                if ctx.args.len() < 2 {
                    return CommandResult::Error("Usage: /reply <message>".to_string());
                }
                let Some(target) = ctx.reply_target.clone() else {
                    return CommandResult::Error("You have nobody to reply to".to_string());
                };
                let message = ctx.args_from(1);

                let online = registry.get_all().await;
                let Some(target_player) = online
                    .iter()
                    .find(|p| p.username.eq_ignore_ascii_case(&target))
                else {
                    return CommandResult::Error(format!("{} is no longer online", target));
                };

                tx.send((
                    ctx.sender.clone(),
                    target_player.username.clone(),
                    message.clone(),
                ))
                .ok();

                let echo = ChatAppender::new()
                    .add(
                        ChatBuilder::new(format!("You -> {}: ", target_player.username))
                            .color(ChatColor::Gray)
                            .italic(),
                    )
                    .add(ChatBuilder::new(&message).color(ChatColor::Gray).italic())
                    .build();
                CommandResult::Success(echo)
            }
        }),
    }
}
