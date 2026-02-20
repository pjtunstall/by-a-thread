use std::sync::mpsc;

use common::player::Color;

use crate::{
    api::{self, ApiError},
    lobby::ui::LobbyUi,
    pre_lobby::state::{ApiRequestPhase, MatchmakerResponse, PreLobby},
    session::ClientSession,
    state::ClientState,
};
use common::auth::Passcode;

const NEW_JOIN_MENU_ITEMS: &[&str] = &["New game", "Join game"];
const JOIN_GUESS_LIMIT: u8 = 3;

pub enum PreLobbyTransition {
    Stay,
    NextState(ClientState),
    Complete(CompleteInfo),
    Exit,
}

pub struct CompleteInfo {
    pub connect_token: renet_netcode::ConnectToken,
    pub server_addr: std::net::SocketAddr,
    pub share_passcode: Option<String>,
    pub only_player: bool,
}

pub fn handle(
    pre_lobby_state: &mut PreLobby,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
) -> PreLobbyTransition {
    let PreLobby::ApiRequestMenu { api_host, phase } = pre_lobby_state else {
        unreachable!();
    };

    if crate::exit::should_quit() {
        return match phase {
            ApiRequestPhase::AwaitingCreate { .. } | ApiRequestPhase::AwaitingJoin { .. } => {
                PreLobbyTransition::NextState(ClientState::PreLobby(PreLobby::ApiRequestMenu {
                    api_host: api_host.to_string(),
                    phase: ApiRequestPhase::ChoosingNewOrJoin {
                        selected_index: 0,
                        prompt_printed: false,
                    },
                }))
            }
            _ => PreLobbyTransition::Exit,
        };
    }

    match phase {
        ApiRequestPhase::ChoosingNewOrJoin {
            selected_index,
            prompt_printed,
        } => handle_choosing_new_or_join(api_host, selected_index, prompt_printed, session, ui),
        ApiRequestPhase::ChoosingPlayerCount { prompt_printed } => {
            handle_choosing_player_count(api_host, prompt_printed, session, ui)
        }
        ApiRequestPhase::ChoosingPasscode {
            wrong_guesses,
            prompt_printed,
        } => handle_choosing_passcode(api_host, *wrong_guesses, prompt_printed, session, ui),
        ApiRequestPhase::AwaitingCreate {
            player_count,
            receiver,
        } => handle_awaiting_create(*player_count, receiver, session, ui),
        ApiRequestPhase::AwaitingJoin {
            passcode,
            wrong_guesses,
            receiver,
        } => handle_awaiting_join(api_host, passcode.clone(), *wrong_guesses, receiver, session, ui),
    }
}

fn handle_choosing_new_or_join(
    api_host: &str,
    selected_index: &mut usize,
    prompt_printed: &mut bool,
    _session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
) -> PreLobbyTransition {
    if !*prompt_printed {
        ui.show_prompt("New game or join game?");
        ui.show_message(" ");
        for (line, color) in &format_new_join_menu_lines(*selected_index) {
            ui.show_message_with_color(line, *color);
        }
        *prompt_printed = true;
        return PreLobbyTransition::Stay;
    }

    match ui.poll_single_key() {
        Ok(Some(common::input::UiKey::Up)) => {
            *selected_index =
                (*selected_index + NEW_JOIN_MENU_ITEMS.len() - 1) % NEW_JOIN_MENU_ITEMS.len();
            let menu_lines = format_new_join_menu_lines(*selected_index);
            ui.replace_last_messages(menu_lines.len(), menu_lines);
            PreLobbyTransition::Stay
        }
        Ok(Some(common::input::UiKey::Down)) => {
            *selected_index = (*selected_index + 1) % NEW_JOIN_MENU_ITEMS.len();
            let menu_lines = format_new_join_menu_lines(*selected_index);
            ui.replace_last_messages(menu_lines.len(), menu_lines);
            PreLobbyTransition::Stay
        }
        Ok(Some(common::input::UiKey::Enter)) => {
            let new_or_join = match *selected_index {
                0 => NewOrJoin::NewGame,
                _ => NewOrJoin::JoinGame,
            };
            let menu_line_count = 1 + format_new_join_menu_lines(0).len();
            ui.replace_last_messages(
                menu_line_count,
                vec![(format!("{}.", new_or_join.menu_label()), Color::WHITE)],
            );
            let phase = match new_or_join {
                NewOrJoin::NewGame => ApiRequestPhase::ChoosingPlayerCount { prompt_printed: false },
                NewOrJoin::JoinGame => ApiRequestPhase::ChoosingPasscode {
                    wrong_guesses: 0,
                    prompt_printed: false,
                },
            };
            PreLobbyTransition::NextState(ClientState::PreLobby(PreLobby::ApiRequestMenu {
                api_host: api_host.to_string(),
                phase,
            }))
        }
        Err(e) => {
            ui.show_sanitized_error(&format!("No connection: {}.", e));
            PreLobbyTransition::Exit
        }
        _ => PreLobbyTransition::Stay,
    }
}

fn handle_choosing_player_count(
    api_host: &str,
    prompt_printed: &mut bool,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
) -> PreLobbyTransition {
    if !*prompt_printed {
        ui.show_prompt("How many players (1-10)? ");
        *prompt_printed = true;
        return PreLobbyTransition::Stay;
    }

    if let Some(input) = session.take_input() {
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            *prompt_printed = false;
            if let Ok(n) = trimmed.parse::<u8>() {
                if (1..=10).contains(&n) {
                    let (tx, rx) = mpsc::channel();
                    let api_host_owned = api_host.to_string();
                    std::thread::spawn(move || {
                        let _ = tx.send(api::create_game(n, Some(&api_host_owned)));
                    });
                    return PreLobbyTransition::NextState(ClientState::PreLobby(PreLobby::ApiRequestMenu {
                        api_host: api_host.to_string(),
                        phase: ApiRequestPhase::AwaitingCreate {
                            player_count: n,
                            receiver: rx,
                        },
                    }));
                }
            }
            ui.show_error("Player count must be 1-10.");
        }
    }
    PreLobbyTransition::Stay
}

fn handle_choosing_passcode(
    api_host: &str,
    wrong_guesses: u8,
    prompt_printed: &mut bool,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
) -> PreLobbyTransition {
    if !*prompt_printed {
        ui.show_prompt("Enter passcode (6 digits): ");
        *prompt_printed = true;
        return PreLobbyTransition::Stay;
    }

    if let Some(input) = session.take_input() {
        let trimmed = input.trim();
        if !trimmed.is_empty() {
            *prompt_printed = false;
            if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
                let (tx, rx) = mpsc::channel();
                let passcode_owned = trimmed.to_string();
                let api_host_owned = api_host.to_string();
                std::thread::spawn(move || {
                    let _ = tx.send(api::join_game(&passcode_owned, Some(&api_host_owned)));
                });
                return PreLobbyTransition::NextState(ClientState::PreLobby(PreLobby::ApiRequestMenu {
                    api_host: api_host.to_string(),
                    phase: ApiRequestPhase::AwaitingJoin {
                        passcode: trimmed.to_string(),
                        wrong_guesses,
                        receiver: rx,
                    },
                }));
            }
            ui.show_error("Passcode must be 6 digits.");
        }
    }
    PreLobbyTransition::Stay
}

fn handle_awaiting_create(
    player_count: u8,
    receiver: &mpsc::Receiver<Result<(api::CreateGameResponse, std::net::SocketAddr), ApiError>>,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
) -> PreLobbyTransition {
    match receiver.try_recv() {
        Ok(Ok((response, addr))) => {
            let token = match crate::net::connect_token_from_base64(&response.connect_token) {
                Ok(t) => t,
                Err(e) => {
                    ui.show_sanitized_error(&e);
                    return PreLobbyTransition::Exit;
                }
            };
            let passcode = match Passcode::from_string(&response.passcode) {
                Some(p) => p,
                None => return PreLobbyTransition::Exit,
            };
            let response = MatchmakerResponse::Create {
                server_addr: addr,
                connect_token: token,
                passcode,
                player_count,
            };
            build_complete(session, response)
        }
        Ok(Err(e)) => {
            ui.show_sanitized_error(&e.to_string());
            PreLobbyTransition::Exit
        }
        Err(mpsc::TryRecvError::Empty) => PreLobbyTransition::Stay,
        Err(mpsc::TryRecvError::Disconnected) => PreLobbyTransition::Exit,
    }
}

fn handle_awaiting_join(
    api_host: &str,
    passcode: String,
    wrong_guesses: u8,
    receiver: &mpsc::Receiver<Result<(api::JoinGameResponse, std::net::SocketAddr), ApiError>>,
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
) -> PreLobbyTransition {
    match receiver.try_recv() {
        Ok(Ok((response, addr))) => {
            let token = match crate::net::connect_token_from_base64(&response.connect_token) {
                Ok(t) => t,
                Err(e) => {
                    ui.show_sanitized_error(&e);
                    return PreLobbyTransition::Exit;
                }
            };
            let passcode = match Passcode::from_string(&passcode) {
                Some(p) => p,
                None => return PreLobbyTransition::Exit,
            };
            let response = MatchmakerResponse::Join {
                server_addr: addr,
                connect_token: token,
                passcode,
            };
            build_complete(session, response)
        }
        Ok(Err(e)) => {
            let is_game_not_found =
                matches!(&e, ApiError::Api { code, .. } if code == "GAME_NOT_FOUND");
            if is_game_not_found {
                let wrong_guesses_after = wrong_guesses + 1;
                if wrong_guesses_after >= JOIN_GUESS_LIMIT {
                    ui.show_sanitized_error(&e.to_string());
                    PreLobbyTransition::NextState(ClientState::PreLobby(PreLobby::ApiRequestMenu {
                        api_host: api_host.to_string(),
                        phase: ApiRequestPhase::ChoosingNewOrJoin {
                            selected_index: 0,
                            prompt_printed: false,
                        },
                    }))
                } else {
                    let remaining = JOIN_GUESS_LIMIT - wrong_guesses_after;
                    ui.show_sanitized_error(&format!(
                        "Wrong passcode. {} {} remaining.",
                        remaining,
                        if remaining == 1 { "guess" } else { "guesses" }
                    ));
                    PreLobbyTransition::NextState(ClientState::PreLobby(PreLobby::ApiRequestMenu {
                        api_host: api_host.to_string(),
                        phase: ApiRequestPhase::ChoosingPasscode {
                            wrong_guesses: wrong_guesses_after,
                            prompt_printed: false,
                        },
                    }))
                }
            } else {
                ui.show_sanitized_error(&e.to_string());
                PreLobbyTransition::NextState(ClientState::PreLobby(PreLobby::ApiRequestMenu {
                    api_host: api_host.to_string(),
                    phase: ApiRequestPhase::ChoosingPasscode {
                        wrong_guesses,
                        prompt_printed: false,
                    },
                }))
            }
        }
        Err(mpsc::TryRecvError::Empty) => PreLobbyTransition::Stay,
        Err(mpsc::TryRecvError::Disconnected) => PreLobbyTransition::Exit,
    }
}

fn build_complete(session: &mut ClientSession, response: MatchmakerResponse) -> PreLobbyTransition {
    let server_addr = response.server_addr();
    let share_passcode = response.share_passcode();
    let only_player = response.only_player();
    let connect_token = response.connect_token();

    session.client_id = connect_token.client_id;
    session.server_addr = Some(server_addr);
    session.transition(ClientState::Lobby(crate::lobby::state::Lobby::Connecting {
        pending_passcode: Some(()),
    }));

    PreLobbyTransition::Complete(CompleteInfo {
        connect_token,
        server_addr,
        share_passcode,
        only_player,
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NewOrJoin {
    NewGame,
    JoinGame,
}

impl NewOrJoin {
    fn menu_label(&self) -> &'static str {
        match self {
            Self::NewGame => "New game",
            Self::JoinGame => "Join game",
        }
    }
}

fn format_new_join_menu_lines(selected_index: usize) -> Vec<(String, Color)> {
    let mut lines = Vec::with_capacity(4);
    for (i, label) in NEW_JOIN_MENU_ITEMS.iter().enumerate() {
        let prefix = if i == selected_index { "  * " } else { "    " };
        let line_color = if i == selected_index {
            Color::WHITE
        } else {
            Color::LightGray
        };
        lines.push((format!("{}{}", prefix, label), line_color));
    }
    lines.push((" ".to_string(), Color::LightGray));
    lines.push((
        "UP/DOWN arrows to select, ENTER to confirm.".to_string(),
        Color::LightGray,
    ));
    lines
}
