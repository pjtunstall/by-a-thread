use std::{net::IpAddr, net::ToSocketAddrs, sync::Arc};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub port_pool: Arc<Mutex<PortPool>>,
    pub server_host: IpAddr,
}

pub struct PortPool {
    ports: Vec<u16>,
}

impl PortPool {
    pub fn new() -> Self {
        Self {
            ports: (7777..=7782).collect(),
        }
    }

    pub fn get(&mut self) -> Option<u16> {
        self.ports.pop()
    }

    pub fn release(&mut self, port: u16) {
        self.ports.push(port);
    }
}

pub type PortPoolState = Arc<Mutex<PortPool>>;

pub fn resolve_server_host(host: &str) -> std::net::IpAddr {
    (host, 0u16)
        .to_socket_addrs()
        .expect("failed to resolve server host")
        .next()
        .expect("server host resolved to no addresses")
        .ip()
}
