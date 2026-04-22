use std::{
    collections::HashMap,
    net::IpAddr,
    time::{Duration, Instant},
};

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
            if let Some(oldest) = entries.first() {
                let elapsed = oldest.elapsed();
                let retry_after = (WINDOW.as_secs() - elapsed.as_secs()).max(1);
                return Err(retry_after);
            }
        }
        entries.push(now);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::net::Ipv4Addr;
    use std::pin::Pin;

    use super::*;

    #[tokio::test]
    async fn check_create_stops_at_limit() {
        check_stops_at_limit(
            |rate_limiter, ip| Box::pin(rate_limiter.check_create(ip)),
            MAX_CREATE_REQUESTS_PER_MINUTE,
        )
        .await;
    }

    #[tokio::test]
    async fn check_join_stops_at_limit() {
        check_stops_at_limit(
            |rate_limiter, ip| Box::pin(rate_limiter.check_join(ip)),
            MAX_JOIN_REQUESTS_PER_MINUTE,
        )
        .await;
    }

    async fn check_stops_at_limit<F>(check_function: F, max_requests_per_minute: usize)
    where
        F: for<'a> Fn(
            &'a RateLimiter,
            IpAddr,
        ) -> Pin<Box<dyn Future<Output = Result<(), u64>> + 'a>>,
    {
        let rate_limiter = RateLimiter::new();
        let ip = IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0));

        for _ in 0..max_requests_per_minute {
            check_function(&rate_limiter, ip)
                .await
                .expect("should not restrict till limit is reached");
        }

        check_function(&rate_limiter, ip)
            .await
            .expect_err("should restrict when limit is reached");
    }
}
