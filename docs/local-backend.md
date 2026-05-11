## How to run the backend locally

- [1. Create environment files](#1-create-environment-files)
- [2. Build the game server image](#2-build-the-game-server-image)
- [3. (Optional) Build the matchmaker image locally](#3-optional-build-the-matchmaker-image-locally)
- [4. Start the stack](#4-start-the-stack)
- [5. Connect the client](#5-connect-the-client)

This guide explains how to run the backend stack (Caddy, matchmaker, and supporting services) on your own machine for testing.

It assumes you have Docker and Docker Compose installed.

### 1. Create environment files

In the project root, create the following files if you haven't already:

- `.env`
- `.env.client`
- `.env.matchmaker`

See `docs/devops.md` (Environment section) for details of the required variables and example values.

For local testing, the key points are:

- In `.env`, set `CADDYFILE=./Caddyfile.local` so that Caddy uses the local configuration with self-signed certificates.
- In `.env.client`, leave `HOST` commented out or unset.
- In `.env.matchmaker`, leave `HOST` commented out or set it to `local`.

### 2. Build the game server image

From the project root, build the game server image used by the matchmaker:

```sh
make server
```

This creates (or updates) the Docker image that will be used for game servers. The name is controlled by `GAME_IMAGE` in `.env.matchmaker`.

### 3. (Optional) Build the matchmaker image locally

By default, `docker compose up` will use the matchmaker image from Docker Hub (`pjtunstall/matchmaker-image:latest`) if it's not present locally. So you can skip this step unless you want to run matchmaker from local source (for example, to test matchmaker changes before they are pushed).

To build and use a local image, from the project root run:

```sh
make matchmaker
```

This builds the matchmaker binary (if needed), then runs `docker build -f matchmaker/Dockerfile -t pjtunstall/matchmaker-image:latest .` so the image is built with the project root as context (the Dockerfile needs `Cargo.toml`, `common/`, and `matchmaker/`). Compose will then use that image when starting the `matchmaker` service.

### 4. Start the stack

From the project root, start the stack with:

```sh
docker compose up -d
```

This will start:

- `socket-proxy` (restricted Docker socket proxy used by the matchmaker),
- `caddy` (reverse proxy and TLS termination),
- `matchmaker` (backend API that spawns and manages game servers).

To stop the stack, run:

```sh
docker compose down
```

### 5. Connect the client

From the project root, for fullscreen mode, run:

```sh
cargo run --release -p client
```

For windowed mode:

```sh
cargo run --release -p client -- --windowed
```

Alternatively, if you've already installed the client from itch.io, you can just launch that. In case you want to edit the assets, see `docs/edit-assets.md`.
