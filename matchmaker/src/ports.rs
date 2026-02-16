use tokio::sync::Mutex;

pub struct PortPool {
    ports: Mutex<Vec<u16>>,
}

impl PortPool {
    pub fn new() -> Self {
        Self {
            ports: Mutex::new((7777..=7786).collect()),
        }
    }

    pub async fn get(&self) -> Option<u16> {
        self.ports.lock().await.pop()
    }

    pub async fn release(&self, port: u16) {
        self.ports.lock().await.push(port);
    }
}
