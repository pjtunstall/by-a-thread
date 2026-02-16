use std::time::Duration;

use bollard::Docker;

use crate::state::AppState;
use common::constants::MAX_SESSION_DURATION;

const CLEANUP_INTERVAL_SECS: u64 = 300;

pub fn spawn_cleanup_task(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(CLEANUP_INTERVAL_SECS));
        interval.tick().await;

        loop {
            interval.tick().await;
            if let Err(e) = run_cleanup(state.clone()).await {
                eprintln!("stale container cleanup failed: {}", e);
            }
        }
    });
}

async fn run_cleanup(state: AppState) -> Result<(), String> {
    let session_duration_with_buffer = Duration::from_secs(MAX_SESSION_DURATION + 60);

    let to_clean: Vec<([u8; 6], String, u16)> = {
        let games = state.games.lock().await;
        games
            .iter()
            .filter_map(|(passcode, game)| {
                let container_name = game.container_name.as_ref()?;
                if game.start_time.elapsed() > session_duration_with_buffer {
                    Some((*passcode, container_name.clone(), game.port))
                } else {
                    None
                }
            })
            .collect()
    };

    if to_clean.is_empty() {
        return Ok(());
    }

    let docker = Docker::connect_with_http_defaults().map_err(|e| e.to_string())?;

    for (_passcode, container_name, _port) in &to_clean {
        if let Err(e) = docker.stop_container(container_name, None).await {
            eprintln!(
                "failed to stop stale container {} (may already have exited): {}",
                container_name, e
            );
        }
    }

    let mut games = state.games.lock().await;
    let mut port_pool = state.port_pool.lock().await;
    for (passcode, _, port) in to_clean {
        games.remove(&passcode);
        port_pool.release(port);
        eprintln!("cleaned up a stale container and released port {}", port);
    }

    Ok(())
}
