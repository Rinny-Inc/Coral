use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Auto,
    Stable,
    Rc,
    Beta,
    Alpha,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum UpdateMode {
    Notify,
    #[default]
    Download,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(untagged)]
pub enum RestartCommand {
    Line(String),
    Argv(Vec<String>),
}
impl Default for RestartCommand {
    fn default() -> Self {
        Self::Line(String::new())
    }
}
impl RestartCommand {
    pub fn argv(&self) -> Option<(String, Vec<String>)> {
        let parts: Vec<String> = match self {
            Self::Argv(parts) => parts.iter().filter(|p| !p.is_empty()).cloned().collect(),
            Self::Line(line) => line.split_whitespace().map(str::to_owned).collect(),
        };
        let mut it = parts.into_iter();
        let program = it.next()?;
        Some((program, it.collect()))
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct UpdaterConfig {
    pub enabled: bool,
    pub channel: UpdateChannel,
    pub mode: UpdateMode,
    pub restart_grace_seconds: u64,
    pub restart_command: RestartCommand,
}
impl Default for UpdaterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            channel: UpdateChannel::Auto,
            mode: UpdateMode::Download,
            restart_grace_seconds: 30,
            restart_command: RestartCommand::default(),
        }
    }
}
