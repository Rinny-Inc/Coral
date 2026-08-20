use std::sync::Arc;

use coral_config::Config;
use coral_server::player::registry::PlayerRegistry;
use tokio::sync::{RwLock, broadcast::Sender};

use crate::{Command, CommandResult, make_handler};

pub fn command(
    config: Arc<RwLock<Config>>,
    player_registry: Arc<PlayerRegistry>,
    difficulty_tx: Arc<Sender<u8>>,
) -> Command {
    Command {
        name: "difficulty",
        aliases: vec![],
        description: "Set the world difficulty",
        usage: "/difficulty <peaceful|easy|normal|hard>",
        handler: make_handler(move |ctx| {
            let config = config.clone();
            let player_registry = player_registry.clone();
            let tx = difficulty_tx.clone();
            async move {
                let Some(arg) = ctx.arg(1) else {
                    return CommandResult::Error(
                        "Usage: /difficulty <peaceful|easy|normal|hard>".to_string(),
                    );
                };

                let (value, name) = match arg.to_lowercase().as_str() {
                    "peaceful" | "0" => (0u8, "Peaceful"),
                    "easy" | "1" => (1u8, "Easy"),
                    "normal" | "2" => (2u8, "Normal"),
                    "hard" | "3" => (3u8, "Hard"),
                    _ => return CommandResult::Error(format!("Unknown dificulty: {}", arg)),
                };

                config.write().await.world.difficulty = value;

                tx.send(value).ok();

                CommandResult::Success(format!("Set the difficulty to {}", name))
            }
        }),
    }
}
