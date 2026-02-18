use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use common::player::Color;

use crate::{
    config,
    lobby::ui::LobbyUi,
    session::ClientSession,
    state::{ClientState, Lobby},
};

const PING_TIMEOUT: Duration = Duration::from_secs(5);
const MATCHMAKER_PORT: u16 = 443;

fn ping_matchmaker(host: &str) -> Result<(), String> {
    let addrs: Vec<_> = (host, MATCHMAKER_PORT)
        .to_socket_addrs()
        .map_err(|e| format!("failed to resolve {}: {}", host, e))?
        .collect();
    for addr in addrs {
        if TcpStream::connect_timeout(&addr, PING_TIMEOUT).is_ok() {
            return Ok(());
        }
    }
    Err(format!("cannot reach {}: connection refused or timed out.", host))
}

fn try_connect_to_host(
    host: String,
    lobby_state: &mut Lobby,
    ui: &mut dyn LobbyUi,
) -> Option<ClientState> {
    let Lobby::ServerAddress { prompt_printed } = lobby_state else {
        unreachable!();
    };
    match ping_matchmaker(&host) {
        Ok(()) => {
            ui.show_message_with_color(
                &format!("Connecting to:\t{}", host),
                Color::WHITE,
            );
            Some(ClientState::Lobby(Lobby::MatchmakerMenu {
                api_host: host,
                prompt_printed: false,
            }))
        }
        Err(e) => {
            ui.show_error(&e);
            ui.show_prompt(&server_address_prompt());
            *prompt_printed = true;
            None
        }
    }
}

pub fn handle(
    lobby_state: &mut Lobby,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
) -> Option<ClientState> {
    let Lobby::ServerAddress { prompt_printed } = lobby_state else {
        unreachable!();
    };

    if let Ok(Some(common::input::UiKey::Tab)) = ui.poll_single_key() {
        session.input_queue.clear();
        return try_connect_to_host(config::LOCAL_MATCHMAKER_HOST.to_string(), lobby_state, ui);
    }

    if let Some(input_string) = session.take_input() {
        let trimmed = input_string.trim();
        if trimmed.is_empty() {
            session.input_queue.clear();
            return try_connect_to_host(config::api_server_host(), lobby_state, ui);
        }
        if let Some(api_host) = validate_matchmaker_host(trimmed) {
            session.input_queue.clear();
            let host = if api_host.eq_ignore_ascii_case("localhost")
                || api_host == "127.0.0.1"
                || api_host == "::1"
            {
                config::LOCAL_MATCHMAKER_HOST.to_string()
            } else {
                api_host
            };
            return try_connect_to_host(host, lobby_state, ui);
        }
        ui.show_error(&server_address_host_error());
        ui.show_prompt(&server_address_prompt());
        *prompt_printed = true;
        return None;
    }

    if !*prompt_printed {
        ui.show_prompt(&server_address_prompt());
        *prompt_printed = true;
        return None;
    }

    None
}

const MAX_HOST_LEN: usize = 253;

fn validate_matchmaker_host(input: &str) -> Option<String> {
    let host = normalize_host_input(input)?;
    if host.len() > MAX_HOST_LEN {
        return None;
    }
    if host.contains(|c: char| c.is_control() || c.is_whitespace()) {
        return None;
    }
    Some(host)
}

fn normalize_host_input(input: &str) -> Option<String> {
    let s = input.trim();
    let lower = s.to_lowercase();
    let without_scheme = if lower.starts_with("https://") {
        &s[8..]
    } else if lower.starts_with("http://") {
        &s[7..]
    } else {
        s
    };
    let (host_part, _) = without_scheme
        .split_once('/')
        .unwrap_or((without_scheme, ""));
    let host = strip_port(host_part.trim())?;
    if host.is_empty() {
        return None;
    }
    Some(host.to_string())
}

fn strip_port(host: &str) -> Option<&str> {
    if host.starts_with('[') {
        let end = host.find(']')?;
        return Some(&host[..=end]);
    }
    if host.matches(':').count() > 1 {
        return Some(host);
    }
    if let Some((h, p)) = host.rsplit_once(':') {
        if p.parse::<u16>().is_ok() && !h.is_empty() {
            return Some(h);
        }
    }
    Some(host)
}

fn server_address_host_error() -> String {
    format!(
        "Invalid host. Enter a domain or IP address (max {} chars).",
        MAX_HOST_LEN
    )
}

fn server_address_prompt() -> String {
    format!(
        "Press Enter for default server (recommended),\nTab if running locally,\nor pick another server (domain or IP address): ",
    )
}
