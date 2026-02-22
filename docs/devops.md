# Devops

- [Environment](#environment)
- [Run locally](#run-locally)
- [Deploy](#deploy)

This is a guide to running the stack with Docker compose, locally or remotely.

## Environment

Create files .env.client and .env.matchmaker in the project root. The client is built with values from .env.client; the matchmaker container reads .env.matchmaker. After entering the following items, run `make check-env` to ensure the two files are consistent.

**.env.client** (used at client build time; values will be baked into the binary on build):

- `CLIENT_PROOF` (required): Base64-encoded secret. The matchmaker validates the client by checking that the SHA-256 hash of the decoded bytes equals `CLIENT_PROOF_HASH` in .env.matchmaker. The secret can be any length; 32 bytes is a typical choice (44 base64 characters). Generate a pair once and use the same `CLIENT_PROOF` in .env.client and the corresponding hex `CLIENT_PROOF_HASH` in .env.matchmaker.

The Renet protocol id is derived from the git commit hash at build time (when available), so client and server built from different commits will reject each other with a clear error. Rebuild both from the same source to fix protocol mismatches.

- `HOST` (optional): Default server for API and game. Omit or leave empty for local. For production, set to your domain (e.g. `HOST=by-a-thread.de`). Must match `HOST` in .env.matchmaker.

**.env.matchmaker** (loaded by Docker Compose for the matchmaker service):

- `CLIENT_PROOF_HASH` (required): 64-character hex string (32 bytes), the SHA-256 hash of the bytes that are base64-encoded as `CLIENT_PROOF` in .env.client.
- `HOST` (optional): Same meaning as in .env.client. Omit or set to `local` or `localhost` for local; set to your domain for production. Must match .env.client (see `make check-env`).

**Local vs production:**

|  | Local | Production |
| --- | --- | --- |
| `.env.client` | Leave `HOST` commented out or unset. | Set `HOST=your-domain.de`. |
| `.env.matchmaker` | Leave `HOST` commented out or set to `local`. | Set `HOST=your-domain.de`. |
| `.env` | Should contain `CADDYFILE=./Caddyfile.local`. | Omit to use default Caddyfile. |

After changing `HOST` for production, rebuild the client so the new default server is baked in.

## Run locally

Build the game server image with make server (builds the server binary and Docker image server-image:latest). Start the compose stack with `docker compose up -d`; stop it with `docker compose down`.

### Notes

To allow the stack to be run locally, I had to take care to satisfy Caddy and Docker:

**Caddy**

1. The API host for local requests must be `localhost`, not `127.0.0.1`, or Caddy rejects the request. When connecting to an IP address, the TLS ClientHello has an empty Server Name Indication (SNI); Caddy refuses connections without SNI because it cannot match the request to a certificate. Using `localhost` sends the hostname in SNI, so Caddy can match the site block.

2. In production, Caddy uses the default Caddyfile which tells it to obtain trusted TLS certificates from Let's Encrypt for `api.by-a-thread.de`. For local development, that's not possible. Create a .env file with `CADDYFILE=./Caddyfile.local` so that Caddy uses Caddyfile.local instead of Caddyfile. Caddyfile.local tells Caddy to use self-signed certs instead of trying to get them from Let's Encrypt. For the sake of local testing, our client accepts these over localhost, and only over localhost.

**Docker**

If the client tries to connect on 127.0.0.1, the server can't reply. When it tries to, it misinterprets the client's 127.0.0.1 (i.e. the address of the host OS) with its own in-container loopback address, and sends the reply to itself.

- Client (on host) sends to 127.0.0.1:7785 from something like 127.0.0.1:45678.
- Docker receives it on the host and DNATs the destination to the container's 5000.
- The source address is usually left as 127.0.0.1:45678.
- The server in the container does recv_from() and sees source 127.0.0.1:45678.
- The server sends the reply to 127.0.0.1:45678.
- Inside the container, 127.0.0.1 is the container's own loopback. The reply goes to the container's loopback, not the host.
- The host client never gets the reply, so the connection times out.

The solution is to make the client connect to the default Docker bridge network, 172.17.0.1. This is the host OS's address as understood both inside and outside the server container.

- Client sends to 172.17.0.1:7785 from something like 172.17.0.1:45678 (same interface).
- Docker forwards to the container; the server sees source 172.17.0.1:45678.
- The server sends the reply to 172.17.0.1:45678.
- From the container, 172.17.0.1 is the gateway to the host. The reply goes out to the host.
- The host receives it on 172.17.0.1 and delivers it to the client.
- The client gets the reply and the connection succeeds.

### Deploy

**Server and Docker**

Provision a cloud server. (I've used Hetzner.) Install Docker.

**SSH**

- SSH Keys: Set up SSH keys for passwordless login. The Makefile deploy target uses the host alias `hetzner`; ensure this is configured in your ~/.ssh/config.
- Docker Hub: Create a Personal Access Token (PAT) on Docker Hub. Log in on the VPS once with `docker login -u <your-username>` to allow it to pull private images.

**Firewall**

Allow SSH (22), HTTP (80) (used by Caddy to set up TLS certificates), HTTPS (443), and UDP on the 10 game server ports 7777–7786.

**Deploy**

The infrastructure is managed via the Makefile. You can run these commands individually, or simply run the final one to trigger the whole chain.

- Build the Images locally. Command: `make build-images`. This runs the release builds for your Rust code and creates the Docker images. It automatically calculates the version from your Cargo.toml for tagging.
- Push to Docker Hub. Command: `make push-images`. This uploads both the "latest" and the versioned tags to your Docker Hub repository. This makes the images accessible to your VPS.
- Deploy to the VPS. Command: `make deploy`. This is the "master" command. It performs the push, then executes the following via SSH on your server:
- Copies docker-compose.yaml, Caddyfile, and .env.matchmaker to the VPS home directory.
- Pulls the latest `server-image`.
- Runs `docker compose up -d --pull always` to refresh the Matchmaker and the network configuration.

**maintainance**

The Makefile also includes the following commands for maintainance:

- `make kill-local-servers` and `make kill-remote-servers` to stop and remove all currently running server containers.
- `make reset-vps` to remove stop the stack from running on Docker compose and remove any game containers.
- `make-clean-local` and `make-clean-remote` to clean up the build environment and Docker images.
- `make-deep-clean-local` and `make-deep-clean-remote` to remove all stopped containers, all unused networks, and all unused images.
