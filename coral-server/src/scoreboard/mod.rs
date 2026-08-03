use std::collections::HashMap;

use tokio::sync::RwLock;

pub mod team;

pub struct ScoreboardManager {
    objectives: RwLock<HashMap<String, (String, String)>>, // name -> (display_name, render_type)
    scores: RwLock<HashMap<(String, String), i32>>,        // (holder, objective) -> value
    sidebar_objective: RwLock<Option<String>>,
}
impl ScoreboardManager {
    pub fn new() -> Self {
        Self {
            objectives: RwLock::new(HashMap::new()),
            scores: RwLock::new(HashMap::new()),
            sidebar_objective: RwLock::new(None),
        }
    }

    pub async fn create_objective(&self, name: &str, display_name: &str) {
        self.objectives.write().await.insert(
            name.to_string(),
            (display_name.to_string(), "integer".to_string()),
        );
    }

    pub async fn set_score(&self, holder: &str, objective: &str, value: i32) {
        self.scores
            .write()
            .await
            .insert((holder.to_string(), objective.to_string()), value);
    }

    pub async fn show_sidebar(&self, objective: &str) {
        *self.sidebar_objective.write().await = Some(objective.to_string());
    }

    /// Returns the packets a caller needs to send to bring a fresh connection up to date
    pub async fn full_state_packets(
        &self,
    ) -> (
        Vec<(String, String, String)>,
        Vec<(String, String, i32)>,
        Option<String>,
    ) {
        let objectives = self
            .objectives
            .read()
            .await
            .iter()
            .map(|(name, (display, render))| (name.clone(), display.clone(), render.clone()))
            .collect();

        let scores = self
            .scores
            .read()
            .await
            .iter()
            .map(|((holder, obj), value)| (holder.clone(), obj.clone(), *value))
            .collect();

        let sidebar = self.sidebar_objective.read().await.clone();
        (objectives, scores, sidebar)
    }
}
