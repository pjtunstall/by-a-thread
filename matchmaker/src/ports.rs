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

    pub async fn release(&self, port: u16) -> Result<(), u16> {
        if !PORT_POOL_RANGE.contains(&port) {
            eprintln!(
                "attempted to release an invalid port: {} (expected {}-{})",
                port, PORT_POOL_START, PORT_POOL_END
            );
            return Err(port);
        }

        let mut ports = self.ports.lock().await;
        if ports.contains(&port) {
            eprintln!(
                "attempted to release a port that is already in the pool: {}",
                port
            );
            return Err(port);
        }

        ports.push(port);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn can_get_available_port() {
        let port_pool = PortPool::new();
        let port = port_pool.get().await;
        assert!(
            port.is_some(),
            "should be able to get a port from a new pool"
        );
    }

    #[tokio::test]
    async fn valid_port_can_be_released() {
        let port_pool = PortPool::new();
        let port = port_pool
            .get()
            .await
            .expect("failed to get a port from a new pool");
        let result = port_pool.release(port).await;
        result.expect(&format!(
            "should be able to free a newly acquired port ({port}) back to the pool"
        ));
    }

    #[tokio::test]
    async fn invalid_port_should_not_be_released() {
        let port_pool = PortPool::new();
        let port = PORT_POOL_END + 1;
        port_pool.release(port).await.expect_err(&format!(
            "should not be able to free a port ({}) outside of the valid range ({}-{})",
            port, PORT_POOL_START, PORT_POOL_END
        ));
    }
}
