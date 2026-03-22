use tokio::sync::Mutex;

const MAX_GAMES: u16 = 10;

pub struct PortPool {
    ports: Mutex<Vec<u16>>,
}

impl PortPool {
    pub fn new() -> Self {
        Self {
            ports: Mutex::new((7777..=(7777 + MAX_GAMES - 1)).collect()),
        }
    }

    pub async fn get(&self) -> Option<u16> {
        self.ports.lock().await.pop()
    }

    pub async fn release(&self, port: u16) {
        if port < 7777 || port > 7777 + MAX_GAMES - 1 {
            eprintln!(
                "attempted to release an invalid port: {} (expected 7777-{})",
                port,
                7777 + MAX_GAMES - 1
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
