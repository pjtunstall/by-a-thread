use std::ops::RangeInclusive;

use tokio::sync::Mutex;

// If these change, be sure to update `docs/devops.md`, `docs/api.yaml`, and the
// firewall rules.
const PORT_POOL_START: u16 = 7777;
const PORT_POOL_SIZE: u16 = 10;
const PORT_POOL_END: u16 = PORT_POOL_START + PORT_POOL_SIZE - 1;
const PORT_POOL_RANGE: RangeInclusive<u16> = PORT_POOL_START..=PORT_POOL_END;

pub struct PortPool {
    ports: Mutex<Vec<u16>>,
}

impl PortPool {
    pub fn new() -> Self {
        Self {
            ports: Mutex::new(PORT_POOL_RANGE.collect()),
        }
    }

    pub async fn get(&self) -> Option<u16> {
        self.ports.lock().await.pop()
    }

    pub async fn release(&self, port: u16) {
        if !PORT_POOL_RANGE.contains(&port) {
            eprintln!(
                "attempted to release an invalid port: {} (expected {}-{})",
                port, PORT_POOL_START, PORT_POOL_END
            );
            return;
        }

        let mut ports = self.ports.lock().await;
        if ports.contains(&port) {
            eprintln!(
                "attempted to release a port that is already in the pool: {}",
                port
            );
            return;
        }

        ports.push(port);
    }
}
