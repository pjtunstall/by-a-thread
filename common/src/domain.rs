// Default Docker bridge network address: This is the host OS's address as
// understood both inside and outside the server container. We use it to run the
// server locally. If the client tries to connect on 127.0.0.1, the server can't
// reply. When it tries to, it misinterprets the client's 127.0.0.1 (i.e. the
// host OS) with its own in-container loopback address, and sends the reply to
// itself.
pub const DOCKER_BRIDGE: &str = "172.17.0.1";

// We have to use `localhost` rather than `127.0.0.1` otherwise Caddy rejects
// it.
fn is_local(value: &str) -> bool {
    let v = value.trim().to_lowercase();
    v.is_empty() || v == "local" || v == "localhost"
}

fn host_from_config(config_value: &str, subdomain: &str) -> String {
    if is_local(config_value) {
        "localhost".to_string()
    } else {
        format!("{}.{}", subdomain, config_value.trim())
    }
}

fn host_config_from_env() -> String {
    std::env::var("HOST").unwrap_or_default()
}

pub fn api_host_from_config(config_value: &str) -> String {
    host_from_config(config_value, "api")
}

pub fn game_host_from_config(config_value: &str) -> String {
    if is_local(config_value) {
        std::env::var("GAME_HOST").unwrap_or_else(|_| DOCKER_BRIDGE.to_string())
    } else {
        format!("game.{}", config_value.trim())
    }
}

pub fn api_server_host() -> String {
    api_host_from_config(&host_config_from_env())
}

pub fn game_server_host() -> String {
    game_host_from_config(&host_config_from_env())
}
