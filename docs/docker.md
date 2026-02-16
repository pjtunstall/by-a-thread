# Docker

- [Matchmaker stack (docker-compose)](#matchmaker-stack-docker-compose)
- [A curiosity: The dummy client trick](#a-curiosity-the-dummy-client-trick)

## Matchmaker stack (docker-compose)

The [docker-compose.yaml](../docker-compose.yaml) stack runs the matchmaker, Caddy, a Docker socket proxy, and Watchtower. The matchmaker service reads env vars from `.env.matchmaker`. That file must provide:

- `VERSION_HASH` (required): a 64-character hex string (32 bytes) that the matchmaker uses to validate client version codes.
- `DOMAIN` (optional): used to resolve the server host for connect tokens (defaults to 127.0.0.1).

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
