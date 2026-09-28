use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use toml_edit::{DocumentMut, Item, Table};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub chat: ChatConfig,
    pub world: WorldConfig,
    pub tracking: TrackingConfig,
    pub bungee: BungeecordConfig,
    pub resource_pack: ResourcePackConfig,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct BungeecordConfig {
    pub enabled: bool,
    pub addresses: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct ServerConfig {
    pub motd: String,
    pub port: u16,
    pub max_players: u32,
    pub online_mode: bool,
    pub player_sample_size: i8,
    pub default_gamemode: u8,
    pub enforce_default_gamemode: bool,
    pub whitelisted: bool,
    pub view_distance: i32,
    pub compression_threshold: i32,
    pub connection_throttle_ms: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct ChatConfig {
    pub format: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct WorldConfig {
    pub world_name: String,
    pub difficulty: u8,
    pub item_despawn_seconds: u64,
    pub disable_weather: bool,
    pub allow_nether: bool,
    pub allow_end: bool,
    pub enable_auto_save: bool,
    pub auto_save_interval: u64,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(default)]
pub struct TrackingConfig {
    pub player: f64,
    pub mob: f64,
    pub item: f64,
    pub experience_orb: f64,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
#[serde(default)]
pub struct ResourcePackConfig {
    pub url: String,
    pub hash: String,
    pub forced: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            motd: "Coral Rust Minecraft Server\nTest Server".to_string(),
            port: 25565,
            max_players: 20,
            online_mode: true,
            player_sample_size: 12,
            default_gamemode: 0,
            enforce_default_gamemode: true,
            whitelisted: false,
            view_distance: 10,
            compression_threshold: 256,
            connection_throttle_ms: 4000,
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
            world_name: "world".to_string(),
            difficulty: 0,
            item_despawn_seconds: 300,
            disable_weather: false,
            allow_nether: true,
            allow_end: true,
            enable_auto_save: true,
            auto_save_interval: 300,
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
impl Default for BungeecordConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            addresses: vec!["127.0.0.1".to_string()],
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config = Self::load_from(Path::new("config.toml"));
        println!("Loaded config: {:#?}", config);
        config
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(content) = fs::read_to_string(path) else {
            println!("[Config] {} not found, creating it..", path.display());
            fs::write(path, DEFAULT_CONFIG.trim_start())
                .unwrap_or_else(|e| eprintln!("[Config] failed to create {}: {e}", path.display()));
            return toml::from_str(DEFAULT_CONFIG).expect("built-in default config is valid");
        };
        let config: Self = match toml::from_str(&content) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[Config] {} could not be parsed: {e}", path.display());
                eprintln!(
                    "[Config] running with built-in defaults for the start! The file has NOT been modified - fix the error above and restart!"
                );
                return Self::default();
            }
        };

        match backfill(&content, &config) {
            Ok(Backfill {
                text,
                added,
                unknown,
            }) => {
                for key in &unknown {
                    eprintln!(
                        "[Config] unknown key `{key}` - ignored, but kept in the file! Typo?"
                    );
                }
                if !added.is_empty() {
                    println!("[Config] adding missing keys: {}", added.join(", "));
                    fs::write(path, &text).unwrap_or_else(|e| {
                        eprintln!("[Config] failed to update {}: {e}", path.display())
                    });
                }
            }
            Err(e) => eprintln!(
                "[Config] could not check {} for missing keys: {e}",
                path.display()
            ),
        }
        config
    }
}

struct Backfill {
    text: String,
    added: Vec<String>,
    unknown: Vec<String>,
}

fn backfill(content: &str, config: &Config) -> Result<Backfill, Box<dyn std::error::Error>> {
    let mut doc: DocumentMut = content.parse()?;
    let reference: DocumentMut = toml::to_string_pretty(config)?.parse()?;

    let mut added = Vec::new();
    let mut unknown = Vec::new();
    merge(
        doc.as_table_mut(),
        reference.as_table(),
        "",
        &mut added,
        &mut unknown,
    );

    Ok(Backfill {
        text: doc.to_string(),
        added,
        unknown,
    })
}
fn merge(
    doc: &mut Table,
    reference: &Table,
    prefix: &str,
    added: &mut Vec<String>,
    unknown: &mut Vec<String>,
) {
    for (key, _) in doc.iter() {
        if reference.get(key).is_none() {
            unknown.push(format!("{prefix}{key}"));
        }
    }
    for (key, ref_item) in reference.iter() {
        match (doc.get_mut(key), ref_item) {
            (Some(Item::Table(sub)), Item::Table(ref_sub)) => {
                merge(sub, ref_sub, &format!("{prefix}{key}"), added, unknown);
            }
            (Some(_), _) => {}
            (None, _) => {
                doc.insert(key, ref_item.clone());
                added.push(format!("{prefix}{key}"));
            }
        }
    }
}

const DEFAULT_CONFIG: &str = r#"
[server]
motd = "Coral Rust Minecraft Server\nTest Server"
port = 25565
max_players = 20
online_mode = true
player_sample_size = 12
default_gamemode = 0
enforce_default_gamemode = true
whitelisted = false
view_distance = 10
compression_threshold = 256
connection_throttle_ms = 4000

[chat]
format = "<{username}> {message}"

[world]
world_name = "world"
difficulty = 0
item_despawn_seconds = 300
disable_weather = false
allow_nether = true
allow_end = true
enable_auto_save = true
# In Seconds
auto_save_interval = 300

[tracking]
player = 512
mob = 80
item = 64
experience_orb = 64

[bungee]
enabled = false
addresses = ["127.0.0.1"]

[resource_pack]
url = ""
hash = "" # optional: sha1sum of the zip for client caching
forced = false
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn write(tag: &str, body: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("coral-config-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, body).unwrap();
        path
    }

    fn read(path: &Path) -> String {
        fs::read_to_string(path).unwrap()
    }

    #[test]
    fn a_partial_section_keeps_its_values() {
        let path = write(
            "partial",
            "[server]\n
             port = 25566\n
             view_distance = 4\n",
        );
        let c = Config::load_from(&path);

        assert_eq!(c.server.port, 25566);
        assert_eq!(c.server.view_distance, 4);
        assert_eq!(c.server.max_players, ServerConfig::default().max_players);
        assert_eq!(c.chat.format, ChatConfig::default().format);

        let after = read(&path);
        assert!(after.contains("port = 25566"));
        assert!(after.contains("view_distance = 4"));
        assert!(
            after.contains("max_players"),
            "missing keys should be added"
        );
    }

    #[test]
    fn comments_and_layout_survive() {
        let path = write(
            "comments",
            "# my server\n
            [server]\n
            port = 25566 # the good port\n
            \n
            [chat]\n
            format = \"{username}: {message}\"\n",
        );
        Config::load_from(&path);
        let after = read(&path);

        assert!(after.contains("# my server"), "leading comment was lost");
        assert!(after.contains("# the good port"), "inline comment was lost");
        assert!(after.contains("{username}: {message}"));
    }

    #[test]
    fn unknown_keys_are_kept() {
        let path = write(
            "unknown",
            "[server]\n
            port = 25566\n
            my_future_setting = true\n
            \n
            [plugins]\n
            foo = 1\n",
        );
        Config::load_from(&path);
        let after = read(&path);

        assert!(after.contains("my_future_setting = true"));
        assert!(after.contains("[plugins]"));
        assert!(after.contains("foo = 1"));
    }

    #[test]
    fn a_broken_file_is_left_alone() {
        let body = "
            [server\n
            port = 25566\n
        ";
        let path = write("broken", body);
        let c = Config::load_from(&path);

        assert_eq!(read(&path), body, "a parse error must not rewrite the file");
        assert_eq!(c.server.port, ServerConfig::default().port);
    }

    #[test]
    fn a_wrongly_typed_value_is_left_alone() {
        let body = "
            [server]\n
            port = \"25566\"\n";
        let path = write("badtype", body);
        Config::load_from(&path);
        assert_eq!(read(&path), body);
    }

    #[test]
    fn a_complete_file_is_not_rewritten() {
        let path = write("complete", DEFAULT_CONFIG.trim_start());
        let before = read(&path);
        Config::load_from(&path);
        assert_eq!(
            read(&path),
            before,
            "load must be idempotent on a full file"
        );
    }

    #[test]
    fn a_missing_file_is_created_with_the_documented_default() {
        let dir = std::env::temp_dir().join(format!("coral-config-new-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");

        let c = Config::load_from(&path);
        assert_eq!(c.server.port, 25565);
        assert!(read(&path).contains("# In Seconds"));
        let before = read(&path);
        Config::load_from(&path);
        assert_eq!(read(&path), before);
    }

    #[test]
    fn an_empty_file_gets_every_section() {
        let path = write("empty", "");
        let c = Config::load_from(&path);
        assert_eq!(c.server.port, 25565);
        let after = read(&path);
        for section in [
            "[server]",
            "[chat]",
            "[world]",
            "[tracking]",
            "[bungee]",
            "[resource_pack]",
        ] {
            assert!(after.contains(section), "{section} was not added");
        }
    }

    #[test]
    fn the_builtin_default_matches_the_struct_defaults() {
        let from_template: Config = toml::from_str(DEFAULT_CONFIG).unwrap();
        let from_struct = Config::default();
        assert_eq!(
            toml::to_string_pretty(&from_template).unwrap(),
            toml::to_string_pretty(&from_struct).unwrap(),
            "DEFAULT_CONFIG has drifted from the Default impls"
        );
    }
}
