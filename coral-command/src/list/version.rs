use coral_protocol::packets::play::chat::builder::{ChatAppender, ChatBuilder, ChatColor};

use crate::{Command, CommandResult, make_handler};

pub fn command() -> Command {
    Command {
        name: "version",
        aliases: vec!["ver"],
        description: "Show Coral version",
        usage: "/version",
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
                .add(ChatBuilder::new(" version ").color(ChatColor::White))
                .add(
                    ChatBuilder::new(format!("git-Coral-{}", env!("GIT_HASH")))
                        .color(ChatColor::LightPurple)
                        .hover_text("Git commit hash"),
                )
                .add(ChatBuilder::new(" (Implementing API version 1.8.x)").color(ChatColor::White))
                .build();
            CommandResult::Success(msg)
        }),
    }
}
