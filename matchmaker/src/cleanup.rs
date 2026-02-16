use std::time::Duration;

use bollard::Docker;

use common::constants::MAX_SESSION_DURATION;
use crate::state::AppState;

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

    let to_clean = state
        .games
        .get_stale_for_cleanup(session_duration_with_buffer)
        .await;

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

    for (passcode, _, port) in to_clean {
        state.games.remove(passcode).await;
        state.port_pool.release(port).await;
        eprintln!("cleaned up a stale container and released port {}", port);
    }

    Ok(())
}
