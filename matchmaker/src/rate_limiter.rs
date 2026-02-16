use std::{collections::HashMap, net::IpAddr, time::{Duration, Instant}};

use tokio::sync::Mutex;

const WINDOW: Duration = Duration::from_secs(60);
const MAX_CREATE_REQUESTS_PER_MINUTE: usize = 4;
const MAX_JOIN_REQUESTS_PER_MINUTE: usize = 10;

pub struct RateLimiter {
    create: Mutex<HashMap<IpAddr, Vec<Instant>>>,
    join: Mutex<HashMap<IpAddr, Vec<Instant>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            create: Mutex::new(HashMap::new()),
            join: Mutex::new(HashMap::new()),
        }
    }

    pub async fn check_create(&self, ip: IpAddr) -> Result<(), u64> {
        self.check(ip, &self.create, MAX_CREATE_REQUESTS_PER_MINUTE)
            .await
    }

    pub async fn check_join(&self, ip: IpAddr) -> Result<(), u64> {
        self.check(ip, &self.join, MAX_JOIN_REQUESTS_PER_MINUTE)
            .await
    }

    async fn check(
        &self,
        ip: IpAddr,
        map: &Mutex<HashMap<IpAddr, Vec<Instant>>>,
        max_per_minute: usize,
    ) -> Result<(), u64> {
        let mut guard = map.lock().await;
        let now = Instant::now();
        let entries = guard.entry(ip).or_default();
        entries.retain(|t| t.elapsed() < WINDOW);
        if entries.len() >= max_per_minute {
            let retry_after = entries
                .first()
                .map(|oldest| {
                    let elapsed = oldest.elapsed();
                    (WINDOW.as_secs() - elapsed.as_secs()).max(1)
                })
                .unwrap_or(WINDOW.as_secs());
            return Err(retry_after);
        }
        entries.push(now);
        Ok(())
    }
}
