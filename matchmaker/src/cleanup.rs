use std::{collections::HashMap, time::Duration};

use bollard::{
    Docker,
    models::ContainerStateStatusEnum,
    query_parameters::{ListContainersOptions, RemoveContainerOptions},
};

use crate::state::AppState;

const CLEANUP_INTERVAL_SECS: u64 = 60;
const DOCKER_WAIT_MAX_ATTEMPTS: u32 = 30;
const DOCKER_WAIT_RETRY_DELAY: Duration = Duration::from_secs(1);

pub async fn wait_for_docker_and_cleanup_zombies() -> Result<(), Box<dyn std::error::Error>> {
    for attempt in 1..=DOCKER_WAIT_MAX_ATTEMPTS {
        match cleanup_zombies().await {
            Ok(()) => return Ok(()),
            Err(e) => {
                if attempt == DOCKER_WAIT_MAX_ATTEMPTS {
                    return Err(e);
                }
                eprintln!(
                    "docker not ready for zombie container cleanup (attempt {}/{}), retrying in {:?}",
                    attempt, DOCKER_WAIT_MAX_ATTEMPTS, DOCKER_WAIT_RETRY_DELAY
                );
                tokio::time::sleep(DOCKER_WAIT_RETRY_DELAY).await;
            }
        }
    }
    unreachable!()
}

pub async fn cleanup_zombies() -> Result<(), Box<dyn std::error::Error>> {
    let docker = Docker::connect_with_http_defaults().map_err(|e| e.to_string())?;
    let mut filters = HashMap::new();
    filters.insert("name".to_string(), vec!["game-".to_string()]);

    let options = Some(ListContainersOptions {
        all: true,
        filters: Some(filters),
        ..Default::default()
    });

    let containers = docker.list_containers(options).await?;

    println!("Connected to Docker.");

    if containers.is_empty() {
        println!("No zombie game containers found.");
        return Ok(());
    }

    for container in containers {
        if let Some(id) = container.id {
            let container_names = container
                .names
                .as_ref()
                .map(|n| n.join(", "))
                .unwrap_or_else(|| "unnamed".to_string());

            let remove_options = Some(RemoveContainerOptions {
                force: true,
                ..Default::default()
            });

            if let Err(e) = docker.remove_container(&id, remove_options).await {
                eprintln!(
                    "failed to cleanup zombie container {} (ID: {}): {}",
                    container_names, id, e
                );
            } else {
                println!("Successfully removed {} (ID: {})", container_names, id);
            }
        }
    }
    Ok(())
}

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
    let candidates = state.games.get_games_with_containers_for_cleanup().await;

    if candidates.is_empty() {
        return Ok(());
    }

    let docker = Docker::connect_with_http_defaults().map_err(|e| e.to_string())?;

    for (passcode, container_name, port, is_time_elapsed) in candidates {
        let mut should_clean = is_time_elapsed;

        if !should_clean {
            match docker.inspect_container(&container_name, None).await {
                Ok(response) => {
                    let is_exited = response
                        .state
                        .as_ref()
                        .and_then(|s| s.status)
                        .is_some_and(|s| matches!(s, ContainerStateStatusEnum::EXITED));
                    if is_exited {
                        should_clean = true;
                    }
                }
                Err(_) => {
                    should_clean = true;
                }
            }
        }

        if should_clean {
            let _ = docker
                .remove_container(
                    &container_name,
                    Some(RemoveContainerOptions {
                        force: true,
                        ..Default::default()
                    }),
                )
                .await;

            state.games.remove(passcode).await;
            state.port_pool.release(port).await;
            println!("Cleaned up {} and released port {}", container_name, port);
        }
    }

    Ok(())
}
