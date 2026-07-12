use crate::{Command, CommandResult, make_handler};

pub fn command() -> Command {
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
