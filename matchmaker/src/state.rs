use std::{collections::HashMap, net::IpAddr, sync::Arc};

use tokio::sync::Mutex;

use crate::{game::Game, ports::PortPool};

#[derive(Clone)]
pub struct AppState {
    pub port_pool: Arc<Mutex<PortPool>>,
    pub server_host: IpAddr,
    pub games: Arc<Mutex<HashMap<[u8; 6], Game>>>,
    pub version_hash: [u8; 32],
}
