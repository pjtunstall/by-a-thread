# Docker

- [Matchmaker stack (docker-compose)](#matchmaker-stack-docker-compose)
- [A curiosity: The dummy package trick](#a-curiosity-the-dummy-package-trick)

## Matchmaker stack (docker-compose)

The [docker-compose.yaml](../docker-compose.yaml) stack runs the matchmaker, Caddy, a Docker socket proxy, and Watchtower. The matchmaker service reads env vars from `.env.matchmaker`. For the full list of variables in `.env.matchmaker` and `.env.client`, and how to set them for local vs production, see [Environment files](architecture.md#environment-files) in the architecture doc. In short: `.env.matchmaker` must provide `VERSION_HASH` (64-char hex) and may provide `HOST` (domain or unset/`local` for local). `HOST` must match `.env.client` (run `make check-env`).

The compose file sets `DOCKER_HOST` and `GAME_IMAGE` directly; override them in the `environment` block if needed. The matchmaker image is built and tagged as `matchmaker-image:latest`; the container runs as `matchmaker-container`. To stop the matchmaker stack, run `docker stop matchmaker-container` (and stop the other services as needed). For running the stack and local development, see [Architecture](architecture.md#matchmaker-and-deployment).

## A curiosity: The dummy package trick

The server consists of one package: `server`. It depends on another package, called `common`. Both belong to the same workspace, and that workspace contains a third package: `client`. I wanted to keep this structure without polluting the Docker build context with the client source code and assets. The solution I found was to include, in my [Dockerfile](../server/Dockerfile), commands to create a dummy client, i.e. the minimal file structure required to satisfy `cargo install`.

```sh
RUN mkdir -p client/src && \
    echo '[package]\nname = "client"\nversion = "0.0.0"\n[dependencies]' > client/Cargo.toml && \
    echo 'fn main() {}' > client/src/main.rs
```

In this way, I could omit/ignore the real client.

The same technique is used in the matchmaker's [Dockerfile](../matchmaker/Dockerfile), with a dummy server and client.
