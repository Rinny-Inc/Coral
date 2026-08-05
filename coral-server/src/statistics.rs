use std::collections::HashMap;

use tokio::sync::RwLock;
use uuid::Uuid;

pub struct StatTracker {
    stats: RwLock<HashMap<Uuid, HashMap<String, i32>>>,
}
impl StatTracker {
    pub fn new() -> Self {
        Self {
            stats: RwLock::new(HashMap::new()),
        }
    }
    pub async fn increment(&self, uuid: Uuid, key: &str, by: i32) -> i32 {
        let mut stats = self.stats.write().await;
        let entry = stats
            .entry(uuid)
            .or_default()
            .entry(key.to_string())
            .or_insert(0);
        *entry += by;
        *entry
    }
}
