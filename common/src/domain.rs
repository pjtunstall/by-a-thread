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
    host_from_config(config_value, "game")
}

pub fn api_server_host() -> String {
    api_host_from_config(&host_config_from_env())
}

pub fn game_server_host() -> String {
    game_host_from_config(&host_config_from_env())
}
