use std::fs;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistEntry {
    uuid: String,
    name: String,
}

pub struct WhitelistFile {
    pub entries: Vec<WhitelistEntry>,
}
impl WhitelistFile {
    pub fn load() -> Self {
        if !fs::exists("whitelist.json").unwrap_or(false) {
            println!("whitelist.json not found, creating empty file..");
            fs::write("whitelist.json", "[]").ok();
            return Self { entries: vec![] };
        }

        let content = fs::read_to_string("whitelist.json").unwrap_or_else(|_| "[]".to_string());
        let entries: Vec<WhitelistEntry> = serde_json::from_str(&content).unwrap_or_else(|e| {
            eprintln!("Failed to parse whitelist.json: {}", e);
            vec![]
        });
        Self { entries }
    }

    pub fn save(&self) {
        let json = serde_json::to_string_pretty(&self.entries).unwrap_or_else(|_| "[]".to_string());
        fs::write("whitelist.json", json)
            .unwrap_or_else(|e| eprintln!("Failed to save whitelist.json {}", e));
    }

    pub fn is_whitelisted(&self, uuid: Uuid) -> bool {
        let uuid_str = uuid.hyphenated().to_string();
        self.entries.iter().any(|e| e.uuid == uuid_str)
    }

    pub fn add(&mut self, uuid: Uuid, name: String) {
        let uuid_str = uuid.hyphenated().to_string();

        self.entries.retain(|e| e.uuid != uuid_str);
        self.entries.push(WhitelistEntry {
            uuid: uuid.to_string(),
            name,
        });
        self.save();
    }

    pub fn remove(&mut self, uuid: Uuid) -> bool {
        let uuid_str = uuid.hyphenated().to_string();
        let before = self.entries.len();

        self.entries.retain(|e| e.uuid != uuid_str);
        if self.entries.len() != before {
            self.save();
            true
        } else {
            false
        }
    }
}
