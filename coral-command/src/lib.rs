use std::{collections::HashMap, pin::Pin, sync::Arc, vec};
use tokio::sync::RwLock;

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
            commands: RwLock::new(HashMap::with_capacity(15)), // TODO: add 1 for every commands
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
