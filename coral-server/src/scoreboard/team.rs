use std::collections::HashMap;

use tokio::sync::RwLock;

pub struct TeamManager {
    teams: RwLock<HashMap<String, Team>>,
}

#[derive(Clone)]
pub struct Team {
    pub name: String,
    pub display_name: String,
    pub prefix: String,
    pub suffix: String,
    pub friendly_fire: bool,
    pub color: u8,
    pub players: Vec<String>,
}

impl TeamManager {
    pub fn new() -> Self {
        Self {
            teams: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_team(&self, name: &str, display_name: &str, color: u8) {
        self.teams.write().await.insert(
            name.to_string(),
            Team {
                name: name.to_string(),
                display_name: display_name.to_string(),
                prefix: String::new(),
                suffix: String::new(),
                friendly_fire: true,
                color,
                players: vec![],
            },
        );
    }

    pub async fn add_player(&self, team_name: &str, player: &str) -> bool {
        let mut teams = self.teams.write().await;
        let Some(team) = teams.get_mut(team_name) else {
            return false;
        };
        if !team.players.contains(&player.to_string()) {
            team.players.push(player.to_string());
        }
        true
    }

    pub async fn blocks_pvp(&self, a: &str, b: &str) -> bool {
        let teams = self.teams.read().await;
        for team in teams.values() {
            if team.players.contains(&a.to_string()) && team.players.contains(&b.to_string()) {
                return !team.friendly_fire;
            }
        }
        false
    }

    pub async fn all_teams(&self) -> Vec<Team> {
        self.teams.read().await.values().cloned().collect()
    }
}
