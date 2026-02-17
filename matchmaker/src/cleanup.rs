use std::time::Duration;

use bollard::{Docker, models::ContainerStateStatusEnum};

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

    let candidates = state
        .games
        .get_games_with_containers_for_cleanup(session_duration_with_buffer)
        .await;

    if candidates.is_empty() {
        return Ok(());
    }

    let docker = Docker::connect_with_http_defaults().map_err(|e| e.to_string())?;

    let mut to_clean = Vec::new();
    for (passcode, container_name, port, is_time_stale) in candidates {
        if is_time_stale {
            to_clean.push((passcode, container_name, port));
            continue;
        }
        let inspect = match docker.inspect_container(&container_name, None).await {
            Ok(i) => i,
            Err(e) => {
                eprintln!(
                    "failed to inspect container {} (may have been removed): {}",
                    container_name, e
                );
                to_clean.push((passcode, container_name, port));
                continue;
            }
        };
        let is_exited = inspect
            .state
            .as_ref()
            .and_then(|s| s.status.as_ref())
            .map_or(false, |s| *s == ContainerStateStatusEnum::EXITED);
        if is_exited {
            to_clean.push((passcode, container_name, port));
        }
    }

    if to_clean.is_empty() {
        return Ok(());
    }

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
