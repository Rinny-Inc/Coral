use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub chat: ChatConfig,
    #[serde(default)]
    pub world: WorldConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    #[serde(default = "default_motd")]
    pub motd: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_max_player")]
    pub max_player: u32,
    #[serde(default = "default_online_mode")]
    pub online_mode: bool,
    #[serde(default = "default_sample_amount")]
    pub player_sample_amount: i8,
    #[serde(default = "default_gamemode")]
    pub default_gamemode: u8,
    #[serde(default = "default_whitelisted")]
    pub whitelisted: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatConfig {
    #[serde(default = "default_chat_format")]
    pub format: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorldConfig {
    #[serde(default = "default_world_difficulty")]
    pub difficulty: u8,
}

fn default_motd() -> String {
    "Coral Rust Minecraft Server\nTest Server".to_string()
}
fn default_port() -> u16 {
    25565
}
fn default_max_player() -> u32 {
    20
}
fn default_online_mode() -> bool {
    true
}
fn default_sample_amount() -> i8 {
    12
}
fn default_gamemode() -> u8 {
    0
}
fn default_whitelisted() -> bool {
    false
}

fn default_chat_format() -> String {
    "<{username}> {message}".to_string()
}

fn default_world_difficulty() -> u8 {
    0
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            motd: default_motd(),
            port: default_port(),
            max_player: default_max_player(),
            online_mode: default_online_mode(),
            player_sample_amount: default_sample_amount(),
            default_gamemode: default_gamemode(),
            whitelisted: default_whitelisted(),
        }
    }
}
impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            format: default_chat_format(),
        }
    }
}
impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            difficulty: default_world_difficulty(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let existed = std::fs::exists("config.toml").unwrap_or(false);

        if !existed {
            println!("config.toml not found, creating default...");
            fs::write("config.toml", DEFAULT_CONFIG.trim_start())
                .unwrap_or_else(|e| eprintln!("Failed to create config.toml: {}", e));
            return toml::from_str(DEFAULT_CONFIG).unwrap();
        }
        let content = fs::read_to_string("config.toml").unwrap_or_else(|_| {
            println!("config.toml not found, using defaults!");
            DEFAULT_CONFIG.to_string()
        });
        let config: Self = toml::from_str(&content).unwrap_or_else(|e| {
            eprintln!("Failed to parse config.toml: {}", e);
            toml::from_str(DEFAULT_CONFIG).unwrap()
        });
        let updated =
            toml::to_string_pretty(&config).unwrap_or_else(|_| DEFAULT_CONFIG.to_string());
        if updated != content {
            println!("config.toml is missing fields, updating with defaults..");
            fs::write("config.toml", &updated)
                .unwrap_or_else(|e| eprintln!("Failed to update config.toml: {}", e));
        }

        config
    }
}

const DEFAULT_CONFIG: &str = r#"
[server]
motd = "Coral Rust Minecraft Server\nTest Server"
port = 25565
max_player = 20
online_mode = true
player_sample_amount = 12
default_gamemode = 0
whitelisted = false

[chat]
format = "<{username}> {message}"

[world]
difficulty = 0
"#;
