# Docker

- [Matchmaker stack (docker-compose)](#matchmaker-stack-docker-compose)
- [A curiosity: The dummy client trick](#a-curiosity-the-dummy-client-trick)

## Matchmaker stack (docker-compose)

The [docker-compose.yaml](../docker-compose.yaml) stack runs the matchmaker, Caddy, a Docker socket proxy, and Watchtower. The matchmaker service reads env vars from `.env.matchmaker`. That file must provide:

- `VERSION_HASH` (required): a 64-character hex string (32 bytes) that the matchmaker uses to validate client version codes.
- `HOST` (optional): deployment target used to resolve the API and game server hosts. Same interpretation as `.env.client` (which bakes the "default server" into the client at build time). Not set or empty resolves to 127.0.0.1. `local` or `localhost` also resolve to 127.0.0.1. For production, set to your domain (e.g. `by-a-thread.de`): HTTP goes to `api.{HOST}`, UDP to `game.{HOST}`. Must match `.env.client` (see `make check-env`).

The compose file sets `DOCKER_HOST` and `GAME_IMAGE` directly; override them in the `environment` block if needed. The matchmaker image is built and tagged as `matchmaker-image:latest`; the container runs as `matchmaker-container`. To stop the matchmaker stack, run `docker stop matchmaker-container` (and stop the other services as needed).

## A curiosity: The dummy package trick

The server consists of one package: `server`. It depends on another package, called `common`. Both belong to the same workspace, and that workspace contains a third package: `client`. I wanted to keep this structure without polluting the Docker build context with the client source code and assets. The solution I found was to include, in my [Dockerfile](../server/Dockerfile), commands to create a dummy client, i.e. the minimal file structure required to satisfy `cargo install`.

```sh
RUN mkdir -p client/src && \
    echo '[package]\nname = "client"\nversion = "0.0.0"\n[dependencies]' > client/Cargo.toml && \
    echo 'fn main() {}' > client/src/main.rs
```

In this way, I could omit/ignore the real client.

The same technique is used in the matchmaker's [Dockerfile](../matchmaker/Dockerfile), with a dummy server and client.

## Localhost

To run locally, we have to take extra care to keep both Caddy and Docker happy.

### Caddy

1. The API host for local requests must be `localhost`, not `127.0.0.1`, or Caddy rejects the request. When connecting to an IP address, the TLS ClientHello has an empty Server Name Indication (SNI); Caddy refuses connections without SNI because it cannot match the request to a certificate. Using `localhost` sends the hostname in SNI, so Caddy can match the site block.

2. In production, Caddy uses the default Caddyfile which tells it to obtain trusted TLS certificates from Let's Encrypt for `api.by-a-thread.de`. For local development, that's not possible. Create a `.env` file with `CADDYFILE=./Caddyfile.local` so that Caddy uses Caddyfile.local instead of Caddyfile. Caddyfile.local tells Caddy to use self-signed certs instead of trying to get them from Let's Encrypt. For the sake of loca testing, our client accepts these over localhost, and only over localhost.

### Docker

The problem: If the client tries to connect on 127.0.0.1, the server can't reply. When it tries to, it misinterprets the client's 127.0.0.1 (i.e. the address of the host OS) with its own in-container loopback address, and sends the reply to itself.

- Client (on host) sends to 127.0.0.1:7785 from something like 127.0.0.1:45678.
- Docker receives it on the host and DNATs the destination to the container’s 5000.
- The source address is usually left as 127.0.0.1:45678.
- The server in the container does recv_from() and sees source 127.0.0.1:45678.
- The server sends the reply to 127.0.0.1:45678.
- Inside the container, 127.0.0.1 is the container's own loopback. The reply goes to the container's loopback, not the host.
- The host client never gets the reply, so the connection times out.

The solution I chose is to make the client connect to the default Docker bridge network, 172.17.0.1. This is the host OS's address as understood both inside and outside the server container.

- Client sends to 172.17.0.1:7785 from something like 172.17.0.1:45678 (same interface).
- Docker forwards to the container; the server sees source 172.17.0.1:45678.
- The server sends the reply to 172.17.0.1:45678.
- From the container, 172.17.0.1 is the gateway to the host. The reply goes out to the host.
- The host receives it on 172.17.0.1 and delivers it to the client.
- The client gets the reply and the connection succeeds.
