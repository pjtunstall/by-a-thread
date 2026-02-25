# DevOps

- [Overview](#overview)
- [Deployment details](#deployment-details)
  - [VPS and Docker Hub](#vps-and-docker-hub)
  - [Caddy and Docker networking](#caddy-and-docker-networking)
  - [Environment](#environment)
  - [GitHub Actions](#github-actions)

This document describes how the backend is hosted in production: where it runs, how images are built and pulled, and how configuration and automation fit together.

For a step-by-step guide to running the backend on your own machine, see `docs/local-backend.md`.

## Overview

The production backend runs on a single VPS (for example, a Hetzner cloud server) running Ubuntu and Docker. A Docker Compose stack manages:

- `watchtower` for automatic updates when new images are available on Docker Hub,
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

Set up SSH keys for passwordless login. The Makefile deploy target uses the host alias `hetzner`; configure it in `~/.ssh/config`.

**Environment**

Create files `.env`, `.env.client`, and `.env.matchmaker` in the project root, as described in [Environment](#environment).

**Deploy**

The stack is driven by the Makefile. Running `make deploy` performs the following steps:

1. **Build images:** Runs release builds and creates the server and matchmaker Docker images (tags from `Cargo.toml`).
2. **Push to Docker Hub:** Pushes both images with `latest` and versioned tags so the VPS can pull them.
3. **Deploy to VPS:** Copies `docker-compose.yaml`, `Caddyfile`, and `.env.matchmaker` to the VPS home directory via SSH, pulls the server image from Docker Hub, instructs Docker Compose to pull the matchmaker and third-party images (for example, Watchtower, Caddy, Docker Socket Proxy), and starts the stack.

### Caddy and Docker networking

This section collects notes about how Caddy and Docker are configured so that TLS and UDP traffic work correctly in both local and production setups.

**Caddy**

1. The API host for local requests must be `localhost`, not `127.0.0.1`, or Caddy rejects the request. When connecting to an IP address, the TLS ClientHello has an empty Server Name Indication (SNI); Caddy refuses connections without SNI because it cannot match the request to a certificate. Using `localhost` sends the hostname in SNI, so Caddy can match the site block.

2. In production, Caddy uses the default `Caddyfile`, which tells it to obtain trusted TLS certificates from Let's Encrypt for `api.by-a-thread.de`. For local development, that is not possible. Set `CADDYFILE=./Caddyfile.local` in `.env` so that Caddy uses `Caddyfile.local` instead of `Caddyfile`. `Caddyfile.local` tells Caddy to use self-signed certs instead of trying to get them from Let's Encrypt.

**Docker**

If the client tries to connect on `127.0.0.1`, the server cannot reply. When it tries to, it misinterprets the client's `127.0.0.1` (the address of the host OS) as its own in-container loopback address, and sends the reply to itself.

- Client (on host) sends to `127.0.0.1:7785` from something like `127.0.0.1:45678`.
- Docker receives it on the host and DNATs the destination to the container's `5000`.
- The source address is usually left as `127.0.0.1:45678`.
- The server in the container does `recv_from()` and sees source `127.0.0.1:45678`.
- The server sends the reply to `127.0.0.1:45678`.
- Inside the container, `127.0.0.1` is the container's own loopback. The reply goes to the container's loopback, not the host.
- The host client never gets the reply, so the connection times out.

The solution is to make the client connect to the default Docker bridge network, `172.17.0.1`. This is the host OS's address as understood both inside and outside the server container.

- Client sends to `172.17.0.1:7785` from something like `172.17.0.1:45678` (same interface).
- Docker forwards to the container; the server sees source `172.17.0.1:45678`.
- The server sends the reply to `172.17.0.1:45678`.
- From the container, `172.17.0.1` is the gateway to the host. The reply goes out to the host.
- The host receives it on `172.17.0.1` and delivers it to the client.
- The client gets the reply and the connection succeeds.

### Environment

Create files `.env.client` and `.env.matchmaker` in the project root. The client is built with values from `.env.client`; the matchmaker container reads `.env.matchmaker`. After entering the following items, run `make check-env` to ensure the two files are consistent.

**.env.client** (used at client build time; values will be baked into the binary on build):

- `HOST`: Default server for API and game. Omit or leave empty for local. For production, set to your domain (for example, `HOST=by-a-thread.de`). Must match `HOST` in `.env.matchmaker`.

- `CLIENT_PROOF`: Base64-encoded secret intended to prove that requests are from a real game client. The matchmaker validates the client by checking that the SHA-256 hash of the decoded bytes equals `CLIENT_PROOF_HASH` in `.env.matchmaker`. The secret can be any length; 32 bytes is a typical choice (44 base64 characters). Generate a pair once (for example, `openssl rand -base64 32` for `CLIENT_PROOF`, then take the SHA-256 hash of the decoded bytes as 64 hex characters for `CLIENT_PROOF_HASH`) and use the same values in both env files.

The Renet protocol id is derived from the Cargo version number at build time. If client and server are built from different versions, the server refuses the connection. Rebuild both from the same source to fix protocol mismatches.

**.env.matchmaker** (loaded by Docker Compose for the matchmaker service):

- `HOST` (optional): Same meaning as in `.env.client`. Omit or set to `local` or `localhost` for local; set to your domain for production. Must match `.env.client` (see `make check-env`).

- `GAME_IMAGE`: The Docker image to use for game servers.

- `CLIENT_PROOF_HASH`: 64-character hex string (32 bytes), the SHA-256 hash of the bytes that are base64-encoded as `CLIENT_PROOF` in `.env.client`. This is used by the matchmaker to validate the client proof.

**Local vs production:**

|  | Local | Production |
| --- | --- | --- |
| `.env.client` | Leave `HOST` commented out or unset. | Set `HOST=your-domain.de`. |
| `.env.matchmaker` | Leave `HOST` commented out or set to `local`. | Set `HOST=your-domain.de`. |
| `.env` | Should contain `CADDYFILE=./Caddyfile.local`. | Omit to use default `Caddyfile`. |

After changing `HOST` for production, rebuild the client so the new default server is baked in.

### GitHub Actions

Two workflows are used to build and deploy the stack.

- **Build**:
  - Builds the client (Windows, Linux, macOS).
  - Builds the backend and deploys it to Docker Hub.
- **Deploy**:
  - Deploys the backend to the VPS on a scheduled basis, or manually if needed.
  - Pushes the client to itch.io.
