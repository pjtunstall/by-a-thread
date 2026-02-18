use std::{collections::HashMap, time::Duration};

use tokio::sync::Mutex;

use crate::{errors::HttpError, game::Game};
use common::constants::LOBBY_TIMER_DURATION;

pub struct Games {
    inner: Mutex<HashMap<[u8; 6], Game>>,
}

impl Games {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub async fn insert(&self, passcode: [u8; 6], game: Game) {
        self.inner.lock().await.insert(passcode, game);
    }

    pub async fn try_join(&self, passcode: [u8; 6]) -> Result<(u16, String), HttpError> {
        let mut guard = self.inner.lock().await;
        let game = guard.get_mut(&passcode).ok_or(HttpError::GameNotFound)?;

        if game.start_time.elapsed() > LOBBY_TIMER_DURATION {
            return Err(HttpError::GameAlreadyStarted);
        }

        let connect_token = game.get_token().ok_or(HttpError::LobbyFull)?;
        let port = game.port;
        Ok((port, connect_token))
    }

    pub async fn get_games_with_containers_for_cleanup(
        &self,
        session_duration: Duration,
    ) -> Vec<([u8; 6], String, u16, bool)> {
        let guard = self.inner.lock().await;
        guard
            .iter()
            .filter_map(|(passcode, game)| {
                let container_name = game.container_name.as_ref()?;
                let is_time_stale = game.start_time.elapsed() > session_duration;
                Some((*passcode, container_name.clone(), game.port, is_time_stale))
            })
            .collect()
    }

    pub async fn remove(&self, passcode: [u8; 6]) {
        self.inner.lock().await.remove(&passcode);
    }
}
