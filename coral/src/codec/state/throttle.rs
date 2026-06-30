use std::{
    collections::HashMap,
    net::IpAddr,
    time::{Duration, Instant},
};

use tokio::sync::RwLock;

pub struct ConnectionThrottle {
    last_connection: RwLock<HashMap<IpAddr, Instant>>,
    throttle_ms: u64,
}
impl ConnectionThrottle {
    pub fn new(throttle_ms: u64) -> Self {
        Self {
            last_connection: RwLock::new(HashMap::new()),
            throttle_ms,
        }
    }

    /// Returns true if the IP should be allowed to connect
    /// Updates the last seen timestamp as a side effect.
    pub async fn check(&self, ip: IpAddr) -> bool {
        if ip.is_loopback() {
            return true; // never throttle local host
        }

        let now = Instant::now();
        let mut map = self.last_connection.write().await;

        if let Some(&last) = map.get(&ip)
            && now.duration_since(last) < Duration::from_millis(self.throttle_ms)
        {
            return false; // nop
        }

        map.insert(ip, now);

        if map.len() > 1000 {
            let cutoff = Duration::from_secs(60);
            map.retain(|_, t| now.duration_since(*t) < cutoff);
        }

        true
    }
}
