use std::sync::Arc;
use tokio::sync::Mutex;

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
