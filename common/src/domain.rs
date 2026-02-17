fn is_local(value: &str) -> bool {
    let v = value.trim().to_lowercase();
    v.is_empty() || v == "local" || v == "localhost"
}

pub fn server_host_from_config(config_value: &str) -> String {
    if is_local(config_value) {
        "127.0.0.1".to_string()
    } else {
        format!("api.{}", config_value.trim())
    }
}

fn host_config_from_env() -> String {
    std::env::var("HOST").unwrap_or_default()
}

pub fn api_server_host() -> String {
    server_host_from_config(&host_config_from_env())
}

pub fn game_server_host() -> String {
    server_host_from_config(&host_config_from_env())
}
