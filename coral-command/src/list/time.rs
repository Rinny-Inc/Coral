use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use coral_protocol::packets::play::chat::builder::{ChatAppender, ChatBuilder};

use crate::{Command, CommandResult, make_handler};

pub fn command(world_time: Arc<AtomicI64>) -> Command {
    Command {
        name: "time",
        aliases: vec![],
        description: "Query or set the world time",
        usage: "/time <query|set|add> <value>",
        handler: make_handler(move |ctx| {
            let world_time = world_time.clone();
            async move {
                if !ctx.is_op {
                    return CommandResult::Error("No permission.".to_string());
                }
                let Some(sub) = ctx.arg(1) else {
                    return CommandResult::Error(
                        "Usage: /time <query|set|add> <value>".to_string(),
                    );
                };

                match sub.to_lowercase().as_str() {
                    "query" => {
                        let t = world_time.load(Ordering::Relaxed);
                        CommandResult::Success(
                            ChatAppender::new()
                                .add(ChatBuilder::new(format!("The time is {}", t)))
                                .build(),
                        )
                    }
                    "set" => {
                        let Some(value_arg) = ctx.arg(2) else {
                            return CommandResult::Error(
                                "Usage: /time set <day|night|noon|midnight|value>".to_string(),
                            );
                        };
                        let new_time = match value_arg.to_lowercase().as_str() {
                            "day" => 1000,
                            "noon" => 6000,
                            "night" => 13000,
                            "midnight" => 18000,
                            other => match other.parse::<i64>() {
                                Ok(v) => v.rem_euclid(24000),
                                Err(_) => {
                                    return CommandResult::Error(format!(
                                        "Invalid time value: {}",
                                        other
                                    ));
                                }
                            },
                        };
                        world_time.store(new_time, Ordering::Relaxed);
                        CommandResult::Success(
                            ChatAppender::new()
                                .add(ChatBuilder::new(format!("Set the time to {}", new_time)))
                                .build(),
                        )
                    }
                    "add" => {
                        let Some(amount_str) = ctx.arg(2) else {
                            return CommandResult::Error("Usage: /time add <amount>".to_string());
                        };
                        let Ok(amount) = amount_str.parse::<i64>() else {
                            return CommandResult::Error(format!("Invalid amount: {}", amount_str));
                        };
                        let current = world_time.load(Ordering::Relaxed);
                        let new_time = (current + amount).rem_euclid(24000);
                        world_time.store(new_time, Ordering::Relaxed);
                        CommandResult::Success(
                            ChatAppender::new()
                                .add(ChatBuilder::new(format!("Added {} to the time", amount)))
                                .build(),
                        )
                    }
                    _ => CommandResult::Error("Usage: /time <query|set|add> <value>".to_string()),
                }
            }
        }),
    }
}
