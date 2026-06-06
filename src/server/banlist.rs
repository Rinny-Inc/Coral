use std::{fs, net::IpAddr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BannedPlayer {
    pub uuid: String,
    pub name: String,
    pub created: String,
    pub source: String,
    pub expires: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BannedIp {
    pub ip: String,
    pub created: String,
    pub source: String,
    pub expires: String,
    pub reason: String,
}

pub struct BanList {
    pub players: Vec<BannedPlayer>,
    pub ips: Vec<BannedIp>,
}

impl BanList {
    pub fn load() -> Self {
        let players = Self::load_players();
        let ips = Self::load_ips();
        println!(
            "Loaded {} banned player(s) and {} banned IP(s)",
            players.len(),
            ips.len()
        );
        Self { players, ips }
    }

    pub fn load_players() -> Vec<BannedPlayer> {
        if !fs::exists("banned-players.json").unwrap_or(false) {
            fs::write("banned-players.json", "[]").ok();
            return vec![];
        }
        let content =
            fs::read_to_string("banned-players.json").unwrap_or_else(|_| "[]".to_string());
        serde_json::from_str(&content).unwrap_or_else(|e| {
            eprintln!("Failed to parse banned-players.json: {}", e);
            vec![]
        })
    }

    pub fn load_ips() -> Vec<BannedIp> {
        if !fs::exists("banned-ips.json").unwrap_or(false) {
            fs::write("banned-ips.json", "[]").ok();
            return vec![];
        }
        let content = fs::read_to_string("banned-ips.json").unwrap_or_else(|_| "[]".to_string());
        serde_json::from_str(&content).unwrap_or_else(|e| {
            eprintln!("Failed to parse banned-ips.json: {}", e);
            vec![]
        })
    }

    fn save_players(&self) {
        let json = serde_json::to_string_pretty(&self.players).unwrap_or_else(|_| "[]".to_string());
        fs::write("banned-players.json", json)
            .unwrap_or_else(|e| eprintln!("Failed to save banned-players.json: {}", e));
    }
    fn save_ips(&self) {
        let json = serde_json::to_string_pretty(&self.ips).unwrap_or_else(|_| "[]".to_string());
        fs::write("banned-ips.json", json)
            .unwrap_or_else(|e| eprintln!("Failed to save banned-ips.json: {}", e));
    }

    fn timestamp() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let dt = secs;
        let s = dt % 60;
        let m = (dt / 60) % 60;
        let h = (dt / 60 * 60) % 24;
        let days = dt / 86400;
        let y = 1970 + days / 365;
        format!("{}-01-01 {:02}:{:02}:{:02} +0000", y, h, m, s)
    }

    pub fn ban_player(&mut self, uuid: Uuid, name: &str, reason: &str, source: &str) {
        self.players
            .retain(|p| p.uuid != uuid.hyphenated().to_string());
        self.players.push(BannedPlayer {
            uuid: uuid.hyphenated().to_string(),
            name: name.to_string(),
            created: Self::timestamp(),
            source: source.to_string(),
            expires: "forever".to_string(),
            reason: reason.to_string(),
        });
        self.save_players();
    }
    pub fn ban_ip(&mut self, ip: IpAddr, reason: &str, source: &str) {
        let ip_str = ip.to_string();
        self.ips.retain(|i| i.ip != ip_str);
        self.ips.push(BannedIp {
            ip: ip_str,
            created: Self::timestamp(),
            source: source.to_string(),
            expires: "forever".to_string(),
            reason: reason.to_string(),
        });
        self.save_ips();
    }

    pub fn unban_player(&mut self, uuid: &Uuid) -> bool {
        let before = self.players.len();
        self.players
            .retain(|p| p.uuid != uuid.hyphenated().to_string());
        if self.players.len() != before {
            self.save_players();
            true
        } else {
            false
        }
    }
    pub fn unban_ip(&mut self, ip: &IpAddr) -> bool {
        let ip_str = ip.to_string();
        let before = self.ips.len();
        self.ips.retain(|i| i.ip != ip_str);
        if self.ips.len() != before {
            self.save_ips();
            true
        } else {
            false
        }
    }

    pub fn is_player_banned(&self, uuid: &Uuid) -> Option<&BannedPlayer> {
        let uuid_str = uuid.hyphenated().to_string();
        self.players.iter().find(|p| p.uuid == uuid_str)
    }
    pub fn is_ip_banned(&self, ip: &IpAddr) -> Option<&BannedIp> {
        let ip_str = ip.to_string();
        self.ips.iter().find(|i| i.ip == ip_str)
    }
}
