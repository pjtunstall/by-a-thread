use std::net::SocketAddr;

use macroquad::prelude::*;
use renet_netcode::ConnectToken;

use crate::{
    api, exit,
    lobby::{state_handlers, ui::LobbyUi},
    session::ClientSession,
    state::{ClientState, InputMode, Lobby},
};
use common::{auth::Passcode, chat::MAX_CHAT_MESSAGE_BYTES, player::Color};

const NEW_JOIN_MENU_ITEMS: &[&str] = &["New game", "Join game"];

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

enum MenuError {
    Fatal,
    ReturnToMenu,
}

pub enum MatchmakerResult {
    Create {
        server_addr: SocketAddr,
        connect_token: ConnectToken,
        passcode: Passcode,
        player_count: u8,
    },
    Join {
        server_addr: SocketAddr,
        connect_token: ConnectToken,
        passcode: Passcode,
    },
}

impl MatchmakerResult {
    pub fn only_player(&self) -> bool {
        match self {
            Self::Create { player_count, .. } => {
                if *player_count == 1 {
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub fn server_addr(&self) -> SocketAddr {
        match self {
            Self::Create { server_addr, .. } | Self::Join { server_addr, .. } => *server_addr,
        }
    }

    pub fn connect_token(self) -> ConnectToken {
        match self {
            Self::Create { connect_token, .. } | Self::Join { connect_token, .. } => connect_token,
        }
    }

    pub fn passcode(&self) -> &Passcode {
        match self {
            Self::Create { passcode, .. } | Self::Join { passcode, .. } => passcode,
        }
    }

    pub fn is_host(&self) -> bool {
        matches!(self, Self::Create { .. })
    }

    pub fn share_passcode(&self) -> Option<String> {
        match self {
            Self::Create { passcode, .. } => Some(passcode.string.clone()),
            Self::Join { .. } => None,
        }
    }
}

pub async fn prompt_for_server_address(
    session: &mut ClientSession,
    ui: &mut dyn LobbyUi,
    font: Option<&macroquad::prelude::Font>,
) -> Option<String> {
    loop {
        if exit::should_quit() {
            return None;
        }

        if let ClientState::Lobby(Lobby::MatchmakerMenu { api_host, .. }) = &session.state {
            ui.flush_input();
            return Some(api_host.clone());
        }

        match ui.poll_input(MAX_CHAT_MESSAGE_BYTES, false) {
            Ok(Some(input)) => session.add_input(input),
            Err(e @ crate::lobby::ui::UiInputError::Disconnected) => {
                ui.show_sanitized_error(&format!("No connection: {}.", e));
                return None;
            }
            Ok(None) => {}
        }

        let state = std::mem::take(&mut session.state);
        let result = match state {
            ClientState::Lobby(mut lobby_state) => {
                let result = match &lobby_state {
                    Lobby::ServerAddress { .. } => {
                        state_handlers::server_address::handle(&mut lobby_state, session, ui)
                    }
                    _ => None,
                };
                session.state = ClientState::Lobby(lobby_state);
                result
            }
            other_state => {
                session.state = other_state;
                None
            }
        };

        if let Some(next_state) = result {
            session.transition(next_state);
        }

        let ui_state = session.prepare_ui_state();
        if ui_state.show_waiting_message {
            ui.show_warning("Waiting for server...");
        }

        let should_show_input = matches!(ui_state.mode, InputMode::Enabled);
        let show_cursor = should_show_input;
        ui.draw(
            should_show_input,
            show_cursor,
            font,
            None::<crate::lobby::ui::LobbyTimerInfo>,
        );

        next_frame().await;
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

async fn choose_new_or_join(
    ui: &mut dyn LobbyUi,
    font: Option<&macroquad::prelude::Font>,
) -> Option<NewOrJoin> {
    let mut prompt_printed = false;
    let mut selected_index: usize = 0;

    loop {
        if exit::should_quit() {
            return None;
        }

        if !prompt_printed {
            ui.show_prompt("New game or join game?");
            ui.show_message(" ");
            for (line, color) in &format_new_join_menu_lines(selected_index) {
                ui.show_message_with_color(line, *color);
            } // Print menu options with current selection.
            prompt_printed = true;
        }

        if let Some(new_or_join) = match handle_menu_navigation(ui, font, &mut selected_index).await
        {
            Err(()) => {
                // TODO: What can go wrong here? Can we be more explicit about
                // that? Should we log the error here, upstream or downstream?
                eprintln!("failed to handle navigation menu");
                return None; // Breaks upstream loop, leading to exit.
            }
            Ok(selection) => selection,
        } {
            return Some(new_or_join);
        }

        if prompt_printed {
            let menu_lines = format_new_join_menu_lines(selected_index);
            ui.replace_last_messages(menu_lines.len(), menu_lines);
        }

        ui.draw(false, false, font, None::<crate::lobby::ui::LobbyTimerInfo>);

        next_frame().await;
    }
}

async fn choose_player_count(
    ui: &mut dyn LobbyUi,
    font: Option<&macroquad::prelude::Font>,
) -> Option<u8> {
    let mut prompt_printed = false;

    loop {
        if exit::should_quit() {
            return None;
        }

        match ui.poll_input(common::chat::MAX_CHAT_MESSAGE_BYTES, false) {
            Ok(Some(input)) => {
                let trimmed = input.trim();
                if !trimmed.is_empty() {
                    if let Ok(n) = trimmed.parse::<u8>() {
                        if (1..=10).contains(&n) {
                            return Some(n);
                        }
                    }
                    ui.show_error("Player count must be 1-10.");
                }
                prompt_printed = false;
            }
            Err(e @ crate::lobby::ui::UiInputError::Disconnected) => {
                ui.show_sanitized_error(&format!("No connection: {}.", e));
                exit::wait_till_escape_is_pressed(ui, font).await;
                return None;
            }
            Ok(None) => {}
        }

        if !prompt_printed {
            ui.show_prompt("How many players (1-10)? ");
            prompt_printed = true;
        }

        ui.draw(true, true, font, None::<crate::lobby::ui::LobbyTimerInfo>);

        next_frame().await;
    }
}

fn remove_new_or_join_menu(ui: &mut dyn LobbyUi, new_or_join: NewOrJoin) {
    let menu_line_count = 1 + format_new_join_menu_lines(0).len();
    ui.replace_last_messages(
        menu_line_count,
        vec![(format!("{}.", new_or_join.menu_label()), Color::WHITE)],
    );
}

async fn choose_passcode(
    ui: &mut dyn LobbyUi,
    font: Option<&macroquad::prelude::Font>,
) -> Option<String> {
    let mut prompt_printed = false;

    loop {
        if exit::should_quit() {
            return None;
        }

        match ui.poll_input(common::chat::MAX_CHAT_MESSAGE_BYTES, false) {
            Ok(Some(input)) => {
                let trimmed = input.trim();
                if trimmed.is_empty() {
                } else if trimmed.len() == 6 && trimmed.chars().all(|c| c.is_ascii_digit()) {
                    return Some(trimmed.to_string());
                } else {
                    ui.show_error("Passcode must be 6 digits.");
                    prompt_printed = false;
                }
            }
            Err(e @ crate::lobby::ui::UiInputError::Disconnected) => {
                ui.show_sanitized_error(&format!("No connection: {}", e));
                exit::wait_till_escape_is_pressed(ui, font).await;
                return None;
            }
            Ok(None) => {}
        }

        if !prompt_printed {
            ui.show_prompt("Enter passcode (6 digits): ");
            prompt_printed = true;
        }

        ui.draw(true, true, font, None::<crate::lobby::ui::LobbyTimerInfo>);

        next_frame().await;
    }
}

pub async fn seek_matchmaker_response(
    api_host: &str,
    ui: &mut dyn LobbyUi,
    font: Option<&macroquad::prelude::Font>,
) -> Option<MatchmakerResult> {
    let matchmaker_host = Some(api_host);
    const JOIN_GUESS_LIMIT: u8 = 3;

    loop {
        ui.flush_input();
        next_frame().await;
        let Some(new_or_join) = choose_new_or_join(ui, font).await else {
            return None;
        };

        remove_new_or_join_menu(ui, new_or_join);

        match new_or_join {
            NewOrJoin::NewGame => {
                ui.flush_input();
                next_frame().await;
                let Some(n) = choose_player_count(ui, font).await else {
                    return None;
                };
                match try_create_game(n, matchmaker_host, ui, font).await {
                    Some(Ok(m)) => return Some(m),
                    Some(Err(MenuError::Fatal)) => {
                        exit::wait_till_escape_is_pressed(ui, font).await;
                        return None;
                    }
                    Some(Err(MenuError::ReturnToMenu)) | None => {}
                }
            }
            NewOrJoin::JoinGame => {
                ui.flush_input();
                next_frame().await;
                for wrong_guesses in 0..JOIN_GUESS_LIMIT {
                    let Some(passcode) = choose_passcode(ui, font).await else {
                        return None;
                    };
                    match try_join_game(
                        &passcode,
                        matchmaker_host,
                        ui,
                        font,
                        wrong_guesses,
                        JOIN_GUESS_LIMIT,
                    )
                    .await
                    {
                        Some(Ok(m)) => return Some(m),
                        Some(Err(MenuError::Fatal)) => {
                            exit::wait_till_escape_is_pressed(ui, font).await;
                            return None;
                        }
                        Some(Err(MenuError::ReturnToMenu)) | None => {}
                    }
                }
            }
        }
    }
}

async fn handle_menu_navigation(
    ui: &mut dyn LobbyUi,
    font: Option<&macroquad::prelude::Font>,
    selected_index: &mut usize,
) -> Result<Option<NewOrJoin>, ()> {
    match ui.poll_single_key() {
        Ok(Some(common::input::UiKey::Up)) => {
            *selected_index =
                (*selected_index + NEW_JOIN_MENU_ITEMS.len() - 1) % NEW_JOIN_MENU_ITEMS.len();
            Ok(None)
        }
        Ok(Some(common::input::UiKey::Down)) => {
            *selected_index = (*selected_index + 1) % NEW_JOIN_MENU_ITEMS.len();
            Ok(None)
        }
        Ok(Some(common::input::UiKey::Enter)) => {
            let selection = match *selected_index {
                0 => NewOrJoin::NewGame,
                _ => NewOrJoin::JoinGame,
            };
            Ok(Some(selection))
        }
        Err(e @ crate::lobby::ui::UiInputError::Disconnected) => {
            ui.show_sanitized_error(&format!("No connection: {}.", e));
            exit::wait_till_escape_is_pressed(ui, font).await;
            Err(())
        }
        _ => Ok(None),
    }
}

async fn try_create_game(
    player_count: u8,
    matchmaker_host: Option<&str>,
    ui: &mut dyn LobbyUi,
    _font: Option<&macroquad::prelude::Font>,
) -> Option<Result<MatchmakerResult, MenuError>> {
    let (response, addr) = match api::create_game(player_count, matchmaker_host) {
        Ok(x) => x,
        Err(e) => {
            ui.show_sanitized_error(&e.to_string());
            return Some(Err(MenuError::Fatal));
        }
    };
    let token = match crate::net::connect_token_from_base64(&response.connect_token) {
        Ok(t) => t,
        Err(e) => {
            ui.show_sanitized_error(&e);
            return Some(Err(MenuError::Fatal));
        }
    };
    let passcode = Passcode::from_string(&response.passcode)?;
    Some(Ok(MatchmakerResult::Create {
        server_addr: addr,
        connect_token: token,
        passcode,
        player_count,
    }))
}

async fn try_join_game(
    trimmed: &str,
    matchmaker_host: Option<&str>,
    ui: &mut dyn LobbyUi,
    _font: Option<&macroquad::prelude::Font>,
    wrong_guesses: u8,
    join_guess_limit: u8,
) -> Option<Result<MatchmakerResult, MenuError>> {
    if trimmed.len() != 6 || !trimmed.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    match api::join_game(trimmed, matchmaker_host) {
        Ok((response, addr)) => {
            let token = match crate::net::connect_token_from_base64(&response.connect_token) {
                Ok(t) => t,
                Err(e) => {
                    ui.show_sanitized_error(&e);
                    return Some(Err(MenuError::Fatal));
                }
            };
            let passcode = Passcode::from_string(trimmed)?;
            Some(Ok(MatchmakerResult::Join {
                server_addr: addr,
                connect_token: token,
                passcode,
            }))
        }
        Err(e) => {
            let is_game_not_found = matches!(
                &e,
                crate::api::ApiError::Api { code, .. } if code == "GAME_NOT_FOUND"
            );
            if is_game_not_found {
                let wrong_guesses_after = wrong_guesses + 1;
                if wrong_guesses_after >= join_guess_limit {
                    ui.show_sanitized_error(&e.to_string());
                    Some(Err(MenuError::ReturnToMenu))
                } else {
                    let remaining = join_guess_limit - wrong_guesses_after;
                    ui.show_sanitized_error(&format!(
                        "Wrong passcode. {} {} remaining.",
                        remaining,
                        if remaining == 1 { "guess" } else { "guesses" }
                    ));
                    None
                }
            } else {
                ui.show_sanitized_error(&e.to_string());
                None
            }
        }
    }
}
