pub const LOCAL_MATCHMAKER_HOST: &str = "localhost";

pub fn api_server_host() -> String {
    common::domain::api_host_from_config(env!("BUILD_HOST"))
}

pub fn game_server_host() -> String {
    common::domain::game_host_from_config(env!("BUILD_HOST"))
}

pub fn version_code() -> &'static str {
    env!("BUILD_VERSION_CODE")
}
