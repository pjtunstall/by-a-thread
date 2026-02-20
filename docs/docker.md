# Docker

- [Matchmaker stack (docker-compose)](#matchmaker-stack-docker-compose)
- [Private Docker Hub registry](#private-docker-hub-registry)
- [A curiosity: The dummy package trick](#a-curiosity-the-dummy-package-trick)

## Matchmaker stack (docker-compose)

The [docker-compose.yaml](../docker-compose.yaml) stack runs the matchmaker, Caddy, a Docker socket proxy, and Watchtower. The matchmaker service reads env vars from `.env.matchmaker`. For the full list of variables in `.env.matchmaker` and `.env.client`, and how to set them for local vs production, see [Environment files](architecture.md#environment-files) in the architecture doc. In short: `.env.matchmaker` must provide `CLIENT_PROOF_HASH` (64-char hex), and may provide `HOST` (domain or unset/`local` for local). `HOST` must match `.env.client` (run `make check-env`).

The compose file sets `DOCKER_HOST` and `GAME_IMAGE` directly; override them in the `environment` block if needed. The matchmaker image is built and tagged as `matchmaker-image:latest`; the container runs as `matchmaker-container`. To stop the matchmaker stack, run `docker stop matchmaker-container` (and stop the other services as needed). For running the stack and local development, see [Architecture](architecture.md#matchmaker-and-deployment).

## Private Docker Hub registry

To use a private Docker Hub repository for the game server and matchmaker images:

1. **Create repositories on Docker Hub.** Sign in at [hub.docker.com](https://hub.docker.com), go to Repositories --> Create Repository, and create two private repositories (e.g. `your-username/game-server` and `your-username/matchmaker`).

2. **Log in from the CLI.** Run `docker login` and enter your Docker Hub username and password (or a [personal access token](https://docs.docker.com/docker-hub/access-tokens/) for password).

3. **Build and tag for Docker Hub.** From the repo root, build then tag with your Hub namespace and repo names:

   ```sh
   make server
   docker tag server-image:latest your-username/server-image:latest
   docker compose build matchmaker
   docker tag matchmaker-image:latest your-username/matchmaker-image:latest
   ```

4. **Push the images.**

   ```sh
   docker push your-username/server-image:latest
   docker push your-username/matchmaker-image:latest
   ```

5. **Use images from the registry.** On a host that has run `docker login` with access to the private repos, pull and run by image name. For the matchmaker stack, set `GAME_IMAGE` to the full server image name so the matchmaker can start game servers from the registry, e.g. in `.env.matchmaker` or by overriding in compose:
   ```yaml
   environment:
     - GAME_IMAGE=your-username/game-server:latest
   ```
   Use `your-username/matchmaker-image:latest` as the matchmaker service image in `docker-compose.yaml` (or override with `docker compose run`) when you want to run the image from Docker Hub instead of building locally.

## A curiosity: The dummy package trick

The server consists of one package: `server`. It depends on another package, called `common`. Both belong to the same workspace, and that workspace contains a third package: `client`. I wanted to keep this structure without polluting the Docker build context with the client source code and assets. The solution I found was to include, in my [Dockerfile](../server/Dockerfile), commands to create a dummy client, i.e. the minimal file structure required to satisfy `cargo install`.

```sh
RUN mkdir -p client/src && \
    echo '[package]\nname = "client"\nversion = "0.0.0"\n[dependencies]' > client/Cargo.toml && \
    echo 'fn main() {}' > client/src/main.rs
```

In this way, I could omit/ignore the real client.

The same technique is used in the matchmaker's [Dockerfile](../matchmaker/Dockerfile), with a dummy server and client.
