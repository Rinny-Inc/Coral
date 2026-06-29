use std::{collections::HashMap, pin::Pin, sync::Arc};

use coral_protocol::packets::play::chat::builder::{ChatAppender, ChatBuilder, ChatColor};
use coral_server::registry::PlayerRegistry;
use coral_types::{GameMode, GamemodeUpdate};
use tokio::sync::{RwLock, broadcast::Sender};

pub type CommandFuture = Pin<Box<dyn Future<Output = CommandResult> + Send>>;
pub type CommandHandler = Arc<dyn Fn(CommandContext) -> CommandFuture + Send + Sync>;

#[derive(Debug, Clone)]
pub struct CommandContext {
    pub sender: String,
    pub args: Vec<String>,
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
    pub name: String,
    pub description: String,
    pub usage: String,
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
        self.commands
            .write()
            .await
            .insert(command.name.clone(), command);
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

pub fn version_command() -> Command {
    Command {
        name: "version".to_string(),
        description: "Show Coral version".to_string(),
        usage: "/version".to_string(),
        handler: make_handler(|_| async move {
            let msg = ChatAppender::new()
                .add(ChatBuilder::new("This server is running ").color(ChatColor::White))
                .add(
                    ChatBuilder::new("Coral")
                        .color(ChatColor::LightPurple)
                        .bold()
                        .click_url("https://github.com/Rinny-Inc/Coral")
                        .hover_text("Open Coral Github Page"),
                )
                .add(ChatBuilder::new(" for Minecraft Protocol 47 (1.8.x)").color(ChatColor::White))
                .build();
            CommandResult::Success(msg)
        }),
    }
}

pub fn list_command(player_registry: Arc<PlayerRegistry>) -> Command {
    Command {
        name: "list".to_string(),
        description: "Show online players list".to_string(),
        usage: "/list".to_string(),
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

// TODO: check if player is op
// then in the future check if the player has the permission
pub fn gamemode_command(
    player_registry: Arc<PlayerRegistry>,
    gm_tx: Arc<Sender<GamemodeUpdate>>,
) -> Command {
    Command {
        name: "gamemode".to_string(),
        description: "Change a player's gamemode".to_string(),
        usage: "/gamemode <mode> [player]".to_string(),
        handler: make_handler(move |ctx| {
            let registry = player_registry.clone();
            let tx = gm_tx.clone();
            async move {
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
                let target_name = ctx.arg(2).unwrap_or(&ctx.sender).to_string();

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
                        .add(ChatBuilder::new(&format!("{:?}", gamemode)).color(ChatColor::White))
                        .build(),
                )
            }
        }),
    }
}
