pub const LOCAL_MATCHMAKER_HOST: &str = "localhost";

pub fn matchmaker_host() -> String {
    common::domain::matchmaker_host_from_config(env!("BUILD_HOST"))
}

pub fn game_server_host() -> String {
    common::domain::game_host_from_config(env!("BUILD_HOST"))
}

pub fn client_proof() -> &'static str {
    env!("BUILD_CLIENT_PROOF")
}
