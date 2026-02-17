use crate::{
    config,
    lobby::ui::LobbyUi,
    session::ClientSession,
    state::{ClientState, Lobby},
};

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
        return Some(ClientState::Lobby(Lobby::MatchmakerMenu {
            api_host: config::LOCAL_MATCHMAKER_HOST.to_string(),
            prompt_printed: false,
        }));
    }

    if let Some(input_string) = session.take_input() {
        let trimmed = input_string.trim();
        if trimmed.is_empty() {
            session.input_queue.clear();
            return Some(ClientState::Lobby(Lobby::MatchmakerMenu {
                api_host: config::api_server_host(),
                prompt_printed: false,
            }));
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
            return Some(ClientState::Lobby(Lobby::MatchmakerMenu {
                api_host: host,
                prompt_printed: false,
            }));
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
    if host.parse::<std::net::IpAddr>().is_ok() {
        return Some(host);
    }
    if looks_like_hostname(&host) {
        return Some(host);
    }
    None
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
    let (host_part, _) = without_scheme.split_once('/').unwrap_or((without_scheme, ""));
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

fn looks_like_hostname(input: &str) -> bool {
    !input.is_empty()
        && input
            .split('.')
            .all(|label| !label.is_empty() && label.len() <= 63 && label.chars().all(hostname_char))
}

fn hostname_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-'
}

fn server_address_host_error() -> String {
    format!(
        "Invalid host. Enter a domain or IP address (max {} chars).",
        MAX_HOST_LEN
    )
}

fn server_address_prompt() -> String {
    format!(
        "Press Enter for default server (recommended),\nTab for localhost,\nor pick another server (domain or IP address): ",
    )
}
