use std::fs;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub chat: ChatConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub motd: String,
    pub port: u16,
    pub max_player: u32,
    pub online_mode: bool,
    pub player_sample_amount: i8,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChatConfig {
    pub format: String,
}

impl Config {
    pub fn load() -> Self {
        let content = fs::read_to_string("config.toml").unwrap_or_else(|_| {
            println!("config.toml not found, using defaults!");
            DEFAULT_CONFIG.to_string()
        });
        toml::from_str(&content).unwrap_or_else(|e| {
            eprintln!("Failed to parse config.toml: {}", e);
            toml::from_str(DEFAULT_CONFIG).unwrap()
        })
    }
}

const DEFAULT_CONFIG: &str = r#"
[server]
motd = "Coral Rust Minecraft Server\nTest Server"
port = 25565
max_player = 20
online_mode = true
player_sample_amount = 12

[chat]
format = "<{username}> {message}"
"#;
