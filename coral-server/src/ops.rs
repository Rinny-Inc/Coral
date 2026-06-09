use std::fs;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpEntry {
    pub uuid: String,
    pub name: String,
    pub level: u8, // 1-4, 4 = full op
}

pub struct OpsFile {
    pub entries: Vec<OpEntry>,
}
impl OpsFile {
    pub fn load() -> Self {
        if !fs::exists("ops.json").unwrap_or(false) {
            println!("ops.json not found, creating empty file..");
            fs::write("ops.json", "[]").ok();
            return Self { entries: vec![] };
        }

        let content = fs::read_to_string("ops.json").unwrap_or_else(|_| "[]".to_string());

        let entries: Vec<OpEntry> = serde_json::from_str(&content).unwrap_or_else(|e| {
            eprintln!("Failed to parse ops.json: {}", e);
            vec![]
        });
        Self { entries }
    }

    pub fn save(&self) {
        let json = serde_json::to_string_pretty(&self.entries).unwrap_or_else(|_| "[]".to_string());
        fs::write("ops.json", json).unwrap_or_else(|e| eprintln!("Failed to save ops.json: {}", e));
    }

    pub fn is_op(&self, uuid: Uuid) -> bool {
        let uuid_str = uuid.hyphenated().to_string();
        self.entries.iter().any(|e| e.uuid == uuid_str)
    }

    pub fn get_level(&self, uuid: Uuid) -> u8 {
        let uuid_str = uuid.hyphenated().to_string();
        self.entries
            .iter()
            .find(|e| e.uuid == uuid_str)
            .map(|e| e.level)
            .unwrap_or(0)
    }

    pub fn add(&mut self, uuid: Uuid, name: &str, level: u8) {
        let uuid_str = uuid.hyphenated().to_string();

        self.entries.retain(|e| e.uuid != uuid_str);
        self.entries.push(OpEntry {
            uuid: uuid_str,
            name: name.to_string(),
            level,
        });
        self.save();
    }

    pub fn remove(&mut self, uuid: &Uuid) -> bool {
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
