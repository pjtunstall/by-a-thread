use bincode::{
    config::standard,
    serde::{decode_from_slice, encode_to_vec},
};

use crate::state::{ClientSession, ClientState};
use crate::{
    net::NetworkHandle,
    ui::{ClientUi, UiInputError},
};
use shared::{
    net::AppChannel,
    {
        input::UiKey,
        protocol::{ClientMessage, ServerMessage},
    },
};

pub fn handle(
    session: &mut ClientSession,
    ui: &mut dyn ClientUi,
    network: &mut dyn NetworkHandle,
) -> Option<ClientState> {
    let is_correct_state = matches!(session.state(), ClientState::ChoosingDifficulty { .. });
    if !is_correct_state {
        panic!(
            "called difficulty::handle() when state was not ChoosingDifficulty; current state: {:?}",
            session.state()
        );
    };

    while let Some(data) = network.receive_message(AppChannel::ReliableOrdered) {
        match decode_from_slice::<ServerMessage, _>(&data, standard()) {
            Ok((
                ServerMessage::CountdownStarted {
                    end_time,
                    maze,
                    players,
                },
                _,
            )) => {
                session.countdown_end_time = Some(end_time);
                session.maze = Some(maze);
                session.players = Some(players);
                return Some(ClientState::Countdown);
            }
            Ok((ServerMessage::ServerInfo { message }, _)) => {
                ui.show_sanitized_message(&format!("Server: {}", message));
                if let ClientState::ChoosingDifficulty { prompt_printed } = session.state_mut() {
                    *prompt_printed = false;
                }
            }
            Ok((_, _)) => {}
            Err(e) => ui.show_sanitized_error(&format!("[Deserialization error: {}]", e)),
        }
    }

    match ui.poll_single_key() {
        Ok(Some(key)) => {
            let level = match key {
                UiKey::Char('1') => Some(1),
                UiKey::Char('2') => Some(2),
                UiKey::Char('3') => Some(3),
                _ => None,
            };

            if let Some(level) = level {
                let msg = ClientMessage::SetDifficulty(level);
                let payload =
                    encode_to_vec(&msg, standard()).expect("failed to serialize SetDifficulty");
                network.send_message(AppChannel::ReliableOrdered, payload);
            }
        }
        Ok(None) => {}
        Err(UiInputError::Disconnected) => {
            return Some(ClientState::Disconnected {
                message: "input disconnected.".to_string(),
            });
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{MockNetwork, MockUi};

    fn setup_tests() -> (ClientSession, MockUi, MockNetwork) {
        let session = ClientSession {
            state: ClientState::ChoosingDifficulty {
                prompt_printed: false,
            },
            ..ClientSession::new(0)
        };
        let ui = MockUi::new();
        let network = MockNetwork::new();
        (session, ui, network)
    }

    #[test]
    fn prints_prompt() {
        let (mut session, mut ui, mut network) = setup_tests();

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(ui.messages.is_empty());
        assert!(ui.prompts.is_empty());
        assert!(matches!(
            session.state(),
            ClientState::ChoosingDifficulty {
                prompt_printed: false
            }
        ));
        assert!(next_state.is_none());
    }

    #[test]
    fn selects_level() {
        let (mut session, mut ui, mut network) = setup_tests();

        handle(&mut session, &mut ui, &mut network);

        ui.keys.push_back(Ok(Some(UiKey::Char('2'))));

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(next_state.is_none());
        assert_eq!(network.sent_messages.len(), 1);

        let (channel, payload) = network.sent_messages.pop_front().unwrap();
        assert_eq!(channel, AppChannel::ReliableOrdered);

        let (msg, _) = decode_from_slice::<ClientMessage, _>(&payload, standard()).unwrap();
        assert_eq!(msg, ClientMessage::SetDifficulty(2));
    }

    #[test]
    fn ignores_invalid_key() {
        let (mut session, mut ui, mut network) = setup_tests();

        handle(&mut session, &mut ui, &mut network);

        ui.keys.push_back(Ok(Some(UiKey::Char('a'))));
        ui.keys.push_back(Ok(Some(UiKey::Enter)));
        ui.keys.push_back(Ok(Some(UiKey::Char('5'))));

        let next_state_1 = handle(&mut session, &mut ui, &mut network);
        let next_state_2 = handle(&mut session, &mut ui, &mut network);
        let next_state_3 = handle(&mut session, &mut ui, &mut network);

        assert!(next_state_1.is_none());
        assert!(next_state_2.is_none());
        assert!(next_state_3.is_none());
        assert!(network.sent_messages.is_empty());
        assert!(ui.errors.is_empty());
    }

    #[test]
    fn disconnects() {
        let (mut session, mut ui, mut network) = setup_tests();

        handle(&mut session, &mut ui, &mut network);

        ui.keys.push_back(Err(UiInputError::Disconnected));

        let next_state = handle(&mut session, &mut ui, &mut network);

        assert!(network.sent_messages.is_empty());
        assert!(matches!(next_state, Some(ClientState::Disconnected { .. })));
    }
}
