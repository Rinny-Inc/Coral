use std::sync::Arc;

use coral_protocol::packets::play::chat::builder::{ChatAppender, ChatBuilder, ChatColor};
use coral_server::player::registry::PlayerRegistry;
use tokio::sync::broadcast::Sender;

use crate::{Command, CommandResult, make_handler};

pub fn command(
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
