use std::sync::Arc;

use coral_protocol::packets::play::chat::builder::{ChatAppender, ChatBuilder, ChatColor};
use coral_server::player::registry::PlayerRegistry;
use coral_types::PrivateMessage;
use tokio::sync::broadcast::Sender;

use crate::{Command, CommandResult, make_handler};

pub fn command(
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
