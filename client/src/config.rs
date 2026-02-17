pub fn api_server_host() -> String {
    common::domain::server_host_from_config(env!("BUILD_HOST"))
}

pub fn game_server_host() -> String {
    common::domain::server_host_from_config(env!("BUILD_HOST"))
}

pub fn version_code() -> &'static str {
    env!("BUILD_VERSION_CODE")
}
