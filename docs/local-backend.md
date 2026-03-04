## How to run the backend locally

This guide explains how to run the backend stack (Caddy, matchmaker, and supporting services) on your own machine for testing.

It assumes you have Docker and Docker Compose installed.

### 1. Create environment files

In the project root, create the following files if you haven't already:

- `.env`
- `.env.client`
- `.env.matchmaker`

See `docs/devops.md` (Environment section) for details of the required variables and example values.

For local testing, the key points are:

- In `.env.client`, leave `HOST` commented out or unset.
- In `.env.matchmaker`, leave `HOST` commented out or set it to `local`.
- In `.env`, set `CADDYFILE=./Caddyfile.local` so that Caddy uses the local configuration with self-signed certificates.

### 2. Build the game server image

From the project root, build the game server image used by the matchmaker:

```bash
make server
```

This creates (or updates) the Docker image that will be used for game servers. The name is controlled by `GAME_IMAGE` in `.env.matchmaker`.

### 3. Build the matchmaker image locally

The `docker-compose.yaml` file refers to the matchmaker container by image name:

```yaml
matchmaker:
  image: pjtunstall/matchmaker-image:latest
```

To run the matchmaker from local source instead of pulling from Docker Hub, build a local image with the same tag.

From the matchmaker backend source directory (the directory containing the `Dockerfile` for the matchmaker), run:

```bash
docker build -t pjtunstall/matchmaker-image:latest .
```

Docker Compose will now use this locally built image when starting the `matchmaker` service.

### 4. Start the stack

From the project root, start the stack with:

```bash
docker compose up -d
```

This will start:

- `socket-proxy` (restricted Docker socket proxy used by the matchmaker),
- `caddy` (reverse proxy and TLS termination),
- `matchmaker` (backend API that spawns and manages game servers).

To stop the stack, run:

```bash
docker compose down
```

### 5. Connect the client

Build the client following `docs/build.md`, ensuring it uses the local configuration (no `HOST` in `.env.client`).

When you launch the client on the same machine, it will, by default:

- connect to the local Caddy instance for API requests, and
- connect to game servers using the UDP address encoded in the connect token, which is the gateway IP of the `back` Docker bridge network (for example, `172.18.0.1`) together with the published game port, as described in `docs/devops.md` (Caddy and Docker section).
