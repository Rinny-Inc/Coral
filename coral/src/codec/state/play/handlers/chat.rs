use coral_command::{CommandContext, CommandResult};
use coral_protocol::packets::play::chat::{
    ChatMessage, ChatMessageOut, TabComplete, TabCompleteResponse,
    builder::{ChatBuilder, ChatColor},
};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

use crate::codec::{
    Codec, PlayerState, send_packet,
    state::play::{PlayCtx, handlers::Flow},
};

pub(in crate::codec::state::play) async fn chat_message(
    chat: ChatMessage,
    framed: &mut Framed<TcpStream, Codec>,
    state: &mut PlayerState,
    ctx: &PlayCtx<'_>,
) -> Flow {
    let max_len = match state.client_brand.as_deref() {
        Some(brand) if brand.contains("forge") || brand.contains("fabric") => 256,
        _ => 100,
    };
    if chat.message.chars().count() > max_len {
        return Flow::Continue;
    }
    if chat.message.starts_with('/') {
        let args: Vec<String> = chat.message[1..]
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        if args.is_empty() {
            return Flow::Continue;
        }

        let command_ctx = CommandContext {
            sender: state.name.to_string(),
            args,
            reply_target: state.last_message_from.clone(),
            is_op: state.is_op,
        };

        match ctx.dispatcher.dispatch(command_ctx).await {
            CommandResult::Success(msg) => {
                send_packet(framed, ChatMessageOut::from_json(&msg)).await;
            }
            CommandResult::Error(msg) => {
                send_packet(
                    framed,
                    ChatBuilder::new(&msg).color(ChatColor::Red).into_packet(),
                )
                .await;
            }
            CommandResult::Broadcast(msg) => {
                ctx.channels
                    .chat_tx
                    .send(ChatBuilder::plain_json(&msg))
                    .ok();
            }
            CommandResult::None => {}
        }
        return Flow::Continue;
    }
    let json = ChatBuilder::chat_message(&ctx.config.chat.format, &state.name, &chat.message);

    ctx.channels.chat_tx.send(json).ok();
    Flow::Continue
}

pub(in crate::codec::state::play) async fn tab_complete(
    tab: TabComplete,
    framed: &mut Framed<TcpStream, Codec>,
    ctx: &PlayCtx<'_>,
) -> Flow {
    let text = &tab.text;
    let matches: Vec<String> = if let Some(partial) = text.strip_prefix('/') {
        ctx.dispatcher.completions(partial).await
    } else {
        ctx.player_registry
            .get_all()
            .await
            .iter()
            .filter(|p| p.username.to_lowercase().starts_with(&text.to_lowercase()))
            .map(|p| p.username.to_string())
            .collect()
    };

    send_packet(framed, TabCompleteResponse { matches }).await;
    Flow::Continue
}
