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
    #[serde(default)]
    pub tracking: TrackingConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    pub motd: String,
    pub port: u16,
    pub max_player: u32,
    pub online_mode: bool,
    pub player_sample_amount: i8,
    pub default_gamemode: u8,
    pub whitelisted: bool,
    pub view_distance: i32,
    pub compression_threshold: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChatConfig {
    pub format: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorldConfig {
    pub difficulty: u8,
    pub item_despawn_seconds: u64,
    pub disable_weather: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TrackingConfig {
    pub player: f64,
    pub mob: f64,
    pub item: f64,
    pub experience_orb: f64,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            motd: "Coral Rust Minecraft Server\nTest Server".to_string(),
            port: 25565,
            max_player: 20,
            online_mode: true,
            player_sample_amount: 12,
            default_gamemode: 0,
            whitelisted: false,
            view_distance: 10,
            compression_threshold: 256,
        }
    }
}
impl Default for ChatConfig {
    fn default() -> Self {
        Self {
            format: "<{username}> {message}".to_string(),
        }
    }
}
impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            difficulty: 0,
            item_despawn_seconds: 300,
            disable_weather: false,
        }
    }
}
impl Default for TrackingConfig {
    fn default() -> Self {
        Self {
            player: 512.0,
            mob: 80.0,
            item: 64.0,
            experience_orb: 64.0,
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
view_distance = 10
compression_threshold = 256

[chat]
format = "<{username}> {message}"

[world]
difficulty = 0
item_despawn_seconds = 300
disable_weather = false

[tracking]
player = 512
mob = 80
item = 64
experience_orb = 64
"#;
