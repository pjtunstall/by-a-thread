# DevOps

- [Overview](#overview)
- [Deployment details](#deployment-details)
  - [VPS and Docker Hub](#vps-and-docker-hub)
  - [Caddy and Docker networking](#caddy-and-docker-networking)
  - [Environment](#environment)
  - [GitHub Actions](#github-actions)
  - [VPS scripts and deployment](#vps-scripts-and-deployment)

This document describes how the backend is hosted in production: where it runs, how images are built and pulled, and how configuration and automation fit together.

For a step-by-step guide to running the backend on your own machine, see `docs/local-backend.md`.

## Overview

The production backend runs on a single VPS (for example, a Hetzner cloud server) running Ubuntu and Docker. A Docker Compose stack manages:

- `socket-proxy` for restricted access to the Docker socket,
- `caddy` as reverse proxy and TLS termination,
- `matchmaker` as the API that spawns and manages game server containers.

Backend images are built from this repository, pushed to Docker Hub, and then pulled by the VPS. GitHub Actions automate most of the build-and-deploy flow.

## Deployment details

### VPS and Docker Hub

**Cloud server**

Provision a cloud server running the latest Ubuntu (for example, a Hetzner VPS) and install Docker.

**Firewall**

Allow SSH (22), HTTP (80) (used by Caddy to set up TLS certificates), HTTPS (443), and UDP on the 10 game server ports 7777–7786.

**Docker Hub**

- Sign in at [hub.docker.com](https://hub.docker.com), go to Repositories --> Create Repository, and create two repositories (for example, `your-username/game-server` and `your-username/matchmaker`). On the free plan, you can only have one private repository, so I made the matchmaker private and the game server public.
- Create a read-write-delete [personal access token](https://docs.docker.com/docker-hub/access-tokens/) (account settings). Use one token locally by running `docker login` and entering it as the password. Create a second token (for example, `vps`) and, via SSH, run `docker login` on the VPS so it can pull images. You only need to log in on the VPS once.

**SSH**

Set up SSH keys for passwordless login. The Makefile targets use the host alias `hetzner`; configure it in `~/.ssh/config` so that `ssh hetzner` logs in as your non-root deploy user.

**Environment**

Create files `.env`, `.env.client`, and `.env.matchmaker` in the project root, as described in [Environment](#environment).

**Deploy**

The stack is driven by the Makefile and by scripts on the VPS. All VPS deployment files live in the home directory of your non-root deploy user (for example, `/home/non-root-user`): `.env.matchmaker`, `Caddyfile`, `docker-compose.yaml`, and a `scripts/` directory containing `deploy_backend.sh` and `deploy_frontend.sh`. Configure SSH so your host alias logs in as that non-root user; then `~` is that directory.

For a fresh VPS, the quickest way to get started is:

1. Configure SSH so `ssh hetzner` logs in as your non-root deploy user.
2. On your development machine, create `.env.matchmaker` in the project root.
3. From the project root, run `make init` to copy `.env.matchmaker` and the three deploy scripts to the VPS and place them under `~/scripts`.
4. In `deploy_frontend.sh`, replace `YOUR_GITHUB_TOKEN` with a GitHub fine-grained pesronal access token so that it can trigger the workflow that pushes client builds to itch.io.
5. On the VPS (logged in as the non-root user), run `docker login` so it can pull images from Docker Hub.
6. Optionally set up cron jobs as described in [VPS scripts and deployment](#vps-scripts-and-deployment).

After initial setup, you can deploy the latest backend and trigger a client deploy by running `make deploy` from your machine. This SSHes to the VPS and runs `deploy_frontend.sh` and `deploy_backend.sh` under `~/scripts` (see [VPS scripts and deployment](#vps-scripts-and-deployment)).

### Caddy and Docker networking

This section collects notes about how Caddy and Docker are configured so that TLS and UDP traffic work correctly in both local and production setups.

**Caddy**

1. The API host for local requests must be `localhost`, not `127.0.0.1`, or Caddy rejects the request. When connecting to an IP address, the TLS ClientHello has an empty Server Name Indication (SNI); Caddy refuses connections without SNI because it cannot match the request to a certificate. Using `localhost` sends the hostname in SNI, so Caddy can match the site block.

2. In production, Caddy uses the default `Caddyfile`, which tells it to obtain trusted TLS certificates from Let's Encrypt for `api.by-a-thread.de`. For local development, that is not possible. Set `CADDYFILE=./Caddyfile.local` in `.env` so that Caddy uses `Caddyfile.local` instead of `Caddyfile`. `Caddyfile.local` tells Caddy to use self-signed certs instead of trying to get them from Let's Encrypt. The client is built to accept these certs when connecting to the matchmaker on localhost.

**Docker**

If the client tries to connect on `127.0.0.1`, the server cannot reply. When it tries to, it misinterprets the client's `127.0.0.1` (the address of the host OS) as its own in-container loopback address, and sends the reply to itself.

- Client (on host) sends to `127.0.0.1:7785` from something like `127.0.0.1:45678`.
- Docker receives it on the host and DNATs the destination to the container's `5000`.
- The source address is usually left as `127.0.0.1:45678`.
- The server in the container does `recv_from()` and sees source `127.0.0.1:45678`.
- The server sends the reply to `127.0.0.1:45678`.
- Inside the container, `127.0.0.1` is the container's own loopback. The reply goes to the container's loopback, not the host.
- The host client never gets the reply, so the connection times out.

The solution is to make the client connect to the host's address on the user-defined bridge network that game servers attach to (the `back` network in `docker-compose.yaml`), rather than to `127.0.0.1` or the Docker default bridge. This address is the network gateway as seen from the containers (for example, something like `172.18.0.1`), and is reachable from both host and containers.

- Client sends to `<back-gateway>:7785` from something like `<back-gateway>:45678` (same interface).
- Docker forwards to the container; the server sees source `<back-gateway>:45678`.
- The server sends the reply to `<back-gateway>:45678`.
- From the container, `<back-gateway>` is the gateway to the host. The reply goes out to the host.
- The host receives it on `<back-gateway>` and delivers it to the client.
- The client gets the reply and the connection succeeds.

To find the exact gateway address on your machine, inspect the `back` network:

```sh
docker network inspect back | jq '.[0].IPAM.Config[0].Gateway'
```

### Environment

Create files `.env.client` and `.env.matchmaker` in the project root. The client is built with values from `.env.client`; the matchmaker container reads `.env.matchmaker`. After entering the following items, run `make check-env` to ensure the two files are consistent. To run locally, also create a file `.env`.

**.env**

To run locally, this should contain:

```sh
CADDYFILE=./Caddyfile.local
```

If using the VPN, this can be omitted or commented out.

**.env.client** (used at client build time; values will be baked into the binary on build):

- `HOST`: Default server for API and game. Omit or leave empty for local. For production, set to your domain (for example, `HOST=by-a-thread.de`). Must match `HOST` in `.env.matchmaker`.

- `CLIENT_PROOF`: Base64-encoded secret intended to prove that requests are from a real game client. The matchmaker validates the client by checking that the SHA-256 hash of the decoded bytes equals `CLIENT_PROOF_HASH` in `.env.matchmaker`. The secret can be any length; 32 bytes is a typical choice (44 base64 characters). Generate a pair once (for example, `openssl rand -base64 32` for `CLIENT_PROOF`, then take the SHA-256 hash of the decoded bytes as 64 hex characters for `CLIENT_PROOF_HASH`) and use the same values in both env files. Or, in Rust:

```rust
use rand::RngExt;
use base64::{Engine as _, engine::general_purpose};
use sha2::{Sha256, Digest};

fn main() {
    let mut rng = rand::rng();
    let random_bytes: [u8; 32] = rng.random();

    let b64_encoded = general_purpose::STANDARD.encode(random_bytes);
    println!("Base64: {}", b64_encoded);

    let mut hasher = Sha256::new();
    hasher.update(random_bytes);
    let hash_result = hasher.finalize();

    println!("SHA-256: {:x}", hash_result);
}
```

The Renet protocol id is derived from the Cargo version number at build time. If client and server are built from different versions, the server refuses the connection. Rebuild both from the same source to fix protocol mismatches.

**.env.matchmaker** (loaded by Docker Compose for the matchmaker service):

- `HOST` (optional): Same meaning as in `.env.client`. Omit or set to `local` or `localhost` for local; set to your domain for production. Must match `.env.client` (see `make check-env`).

- `GAME_IMAGE`: The Docker image to use for game servers: `pjtunstall/server-image:latest`.

- `CLIENT_PROOF_HASH`: 64-character hex string (32 bytes), the SHA-256 hash of the bytes that are base64-encoded as `CLIENT_PROOF` in `.env.client`. This is used by the matchmaker to validate the client proof.

**Local vs production:**

|  | Local | Production |
| --- | --- | --- |
| `.env` | Should contain `CADDYFILE=./Caddyfile.local`. | Omit to use default `./Caddyfile`. |
| `.env.client` | Leave `HOST` commented out or unset. | Set `HOST=your-domain.de`. |
| `.env.matchmaker` | Leave `HOST` commented out or set to `local`. | Set `HOST=your-domain.de`. |

After changing `HOST` for production, rebuild the client so the new default server is baked in.

### GitHub Actions

Two workflows are used to build and deploy the stack.

- **Build (`build.yaml`)**:
  - Runs tests.
  - Builds backend Docker images and pushes them to Docker Hub.
  - Builds client artifacts for Windows, Linux (`.deb`, `.rpm`, AppImage), and macOS, and uploads them as GitHub Actions artifacts.
- **Deploy (`deploy.yaml`)**:
  - Downloads the client artifacts produced by the build workflow.
  - Uses Butler to push the client builds to itch.io (one channel per platform/package type).

These workflows rely on a set of GitHub secrets to populate `.env` files during builds and to authenticate to external services:

- **`HOST`**
  - Used in `build.yaml` to populate `HOST` in `.env.client` and `.env.matchmaker`.
  - Controls the default API and game host baked into the client binaries and used by the matchmaker in CI builds.

- **`CLIENT_PROOF`**
  - Used in `build.yaml` to populate `CLIENT_PROOF` in `.env.client`.
  - Baked into the client binaries as the base64-encoded secret that proves requests come from an authentic client.

- **`CLIENT_PROOF_HASH`**
  - Used in `build.yaml` to populate `CLIENT_PROOF_HASH` in `.env.matchmaker`.
  - Configures the hash that the matchmaker uses to validate the client proof header.

- **`DOCKERHUB_USERNAME`**
  - Used in `build.yaml` to:
    - Log in to Docker Hub via `docker/login-action@v3`.
    - Populate `GAME_IMAGE` in `.env.matchmaker` as `${DOCKERHUB_USERNAME}/server-image:latest`, so the matchmaker launches game servers from the correct image.

- **`DOCKERHUB_TOKEN`**
  - Used in `build.yaml` as the password for `docker/login-action@v3`.
  - Grants the workflow permission to push `server-image` and `matchmaker-image` tags to Docker Hub.

- **`BUTLER_API_KEY`**
  - Used in `deploy.yaml` as `BUTLER_API_KEY`, the authentication token Butler uses to push builds to itch.io.

- **`ITCH_USERNAME`** and **`ITCH_GAME`**
  - Combined in `deploy.yaml` into `GAME=${{ secrets.ITCH_USERNAME }}/${{ secrets.ITCH_GAME }}`.
  - Identify the itch.io game target for Butler channels (`:windows`, `:linux`, `:linux-deb`, `:linux-rpm`, `:mac-*`).

The `Deploy` workflow can be started manually from the GitHub UI (`workflow_dispatch`) or triggered by a `repository_dispatch` event of type `deploy_itch`. A script running on the VPS (or any other trusted system) can call the GitHub REST API with an appropriate personal access token to send that event, which in turn causes `deploy.yaml` to run and push the latest client artifacts to itch.io.

WARNING: If deploying manually from the GitHub UI, be sure not to let the client get out of sync with the backend!

### VPS scripts and deployment

The repo is not cloned on the VPS. For initial setup and deployment, follow the quick-start steps in the [Deploy](#deploy) section above (`make init`, `docker login`). Significant files and folders are descibed in the following two sections. Optionally set up cron jobs to trigger automatic updates as described below in [Scheduled maintenance](#scheduled-maintenance).

**Deploy directory**

- `~/` (for example, `/home/non-root-user`) is the deploy directory, containing `.env.matchmaker`, `Caddyfile`, and `docker-compose.yaml` (the latest versions of the last two are fetched by `deploy_backend.sh` from the GitHub repo).

**Scripts:**

Three scripts run nightly on the VPS to deploy the latest back and front ends:

- **`deploy_frontend.sh`** triggers the `Deploy` GitHub workflow that pushes client builds to itch.io (via `repository_dispatch`). It runs from cron as `non-root-user` at 04:01.
- **`deploy_backend.sh`** fetches `docker-compose.yaml` and `Caddyfile` from GitHub (`main`) into `/home/non-root-user`, pulls the server image from Dokcer Hub, and runs `docker compose up -d` there. It runs from cron as `non-root-user` at 04:30, then reboots if `/var/run/reboot-required` exists.

**Manual deploy on the VPS:**

Updates can also be triggered manually on the VPS thus:

```bash
cd ~/scripts
./deploy_frontend.sh   # trigger client → itch.io
./deploy_backend.sh    # fetch compose/Caddyfile, pull images, start stack
```

or from your development machine:

```bash
make deploy
```

### Scheduled maintenance

Maintainance is scheduled to run between 04:00 and 05:00 UTC. Twenty minutes before disruptive actions, the matchmaker stops accepting new-game requests. It resumes when maintenance is expected to be complete.

| **Time (UTC)** | **Action** | **Triggered By** |
| --- | --- | --- |
| 04:00 | Matchmaker locks | Matchmaker clock |
| 04:01 | Webhook Fires to deploy client artifacts to itch.io | deploy_frontend.sh (Cron, root user) |
| 04:01 | OS Package Lists Downloaded | apt-daily.timer (Systemd) |
| 04:20 | OS Security Patches Installed | apt-daily-upgrade.timer (Systemd) |
| 04:30 | Docker Images Updated; reboot if needed | deploy_backend.sh (Cron, non-root-user) |
| 05:00 | Matchmaker Unlocks | Matchmaker clock |

For the sake of simplicity, I chose to let the matchmaker initiate lock/unlock for now. Ideally, though, there would be a single source of truth to synchronize the whole maintenance sequence. One suggestion is to have a script on the VPS create and remove a sentinel file to indicate that maintenance is in progress. The matchmaker would check for the existence of this file before accepting a new-game request. That would ensure that the matchmaker knows the correct state even when it restarts after the VPS reboots.
