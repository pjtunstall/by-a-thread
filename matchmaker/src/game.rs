use std::collections::HashMap;

use std::time::Instant;

use base64::Engine;
use bollard::{
    Docker,
    models::{ContainerCreateBody, ContainerStateStatusEnum, HostConfig, PortBinding},
    query_parameters::CreateContainerOptions,
};
use uuid::Uuid;

use crate::{auth, errors::HttpError};
use common::{auth::Passcode, constants::SERVER_PORT};

#[derive(Debug)]
pub struct Game {
    pub id: Uuid,
    pub port: u16,
    pub player_count: u8,
    pub connect_tokens: Vec<String>,
    pub start_time: Instant,
    pub container_name: Option<String>,
    pub passcode: Passcode,
}

impl Game {
    pub fn new(
        server_host: std::net::IpAddr,
        port: u16,
        player_count: u8,
        private_key: [u8; 32],
        passcode: Passcode,
    ) -> Self {
        let mut connect_tokens = Vec::new();

        for _ in 0..player_count {
            let connect_token = auth::create_connect_token(server_host, port, &private_key);
            let mut bytes = Vec::new();
            connect_token
                .write(&mut bytes)
                .expect("failed to write token");
            let connect_token_str = base64::engine::general_purpose::STANDARD.encode(&bytes);
            connect_tokens.push(connect_token_str);
        }

        Self {
            id: Uuid::new_v4(),
            port,
            player_count,
            connect_tokens,
            start_time: Instant::now(),
            container_name: None,
            passcode,
        }
    }

    pub fn get_token(&mut self) -> Option<String> {
        self.connect_tokens.pop()
    }

    pub async fn start_server_container(
        &mut self,
        private_key: [u8; 32],
        server_host: std::net::IpAddr,
    ) -> Result<(), HttpError> {
        let docker = Docker::connect_with_http_defaults().map_err(|e| {
            eprintln!("docker connect failed: {}", e);
            HttpError::ServerError
        })?;
        let container_name = format!("game-{}", uuid::Uuid::new_v4());
        let private_key_b64 = base64::engine::general_purpose::STANDARD.encode(&private_key);

        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            format!("{SERVER_PORT}/udp"),
            Some(vec![PortBinding {
                host_ip: None,
                host_port: Some(self.port.to_string()),
            }]),
        );

        let config = ContainerCreateBody {
            image: Some(
                std::env::var("GAME_IMAGE").unwrap_or_else(|_| "server-image:latest".to_string()),
            ),
            env: Some(vec![
                format!("PRIVATE_KEY={}", private_key_b64),
                format!("IP={}", server_host),
                format!("PORT={}", self.port),
                format!("PASSCODE={}", self.passcode.string),
            ]),
            host_config: Some(HostConfig {
                port_bindings: Some(port_bindings),
                ..Default::default()
            }),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name: Some(container_name.clone()),
            ..Default::default()
        };
        docker
            .create_container(Some(options), config)
            .await
            .map_err(|e| {
                eprintln!("docker create_container failed: {}", e);
                HttpError::ServerError
            })?;

        docker
            .start_container(&container_name, None)
            .await
            .map_err(|e| {
                eprintln!("docker start_container failed: {}", e);
                HttpError::ServerError
            })?;

        self.container_name = Some(container_name.clone());

        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        let inspect = docker
            .inspect_container(&container_name, None)
            .await
            .map_err(|e| {
                eprintln!("docker inspect_container failed: {}", e);
                HttpError::ServerError
            })?;
        if inspect
            .state
            .as_ref()
            .and_then(|s| s.status)
            .map_or(false, |s| s == ContainerStateStatusEnum::EXITED)
        {
            eprintln!(
                "game server container {} exited immediately; check container logs",
                container_name
            );
            if let Err(e) = docker.remove_container(&container_name, None).await {
                eprintln!(
                    "failed to remove exited container {}: {}",
                    container_name, e
                );
            }
            return Err(HttpError::ServerError);
        }

        Ok(())
    }
}
