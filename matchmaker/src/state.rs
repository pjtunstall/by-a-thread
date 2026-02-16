use std::{net::IpAddr, sync::Arc};

use crate::{games::Games, ports::PortPool, rate_limiter::RateLimiter};

#[derive(Clone)]
pub struct AppState {
    pub port_pool: Arc<PortPool>,
    pub server_host: IpAddr,
    pub games: Arc<Games>,
    pub version_hash: [u8; 32],
    pub rate_limiter: Arc<RateLimiter>,
}
