# Docker

- [To run the server on Docker](#to-run-the-server-on-docker)
- [The dummy client trick](#the-dummy-client-trick)

## To run the server on Docker

Assuming you've installed [Docker](https://www.docker.com/) and started it with `sudo systemctl start docker` if need be, you can run the server on Docker and the client directly on your host machine.

The following commands build a docker image of the server, run the corresponding container in the background, and print log initial output, including, most importantly, the ephemeral passcode that clients must enter to be admitted to a game. They should be run from the root directory of the project.

```sh
docker build -t server-image .
docker run -d \
  --name server-container \
  --rm \
  -p 5000:5000/udp \
  server-image
docker logs server-container # To see the server banner with the passcode.
```

Alternatively, you can run the same commands as `./docker_script.sh` after first making it executable: `chmod +x docker_script.sh`.[^1] You can then launch clients in the usual way: `cargo run --release --bin client` (except that one of them can be in the same terminal as the server now). To stop the container:

```sh
docker stop server-container
```

Or, if you wait a few seconds after the last client disconnected, the server will exit and then the container will stop of its own accord. The `--rm` flag with the `run` command ensures that the container will be deleted when it stops.

## The dummy client trick

As I containerized the server using Docker, I came across a useful trick. The server consists of one package: `server`. It depends on another package, called `common`. Both belong to the same workspace, and that workspace contains a third package: `client`. I wanted to keep this structure without polluting the Docker build context with the client source code and assets. The solution I found was to include, in my [Dockerfile](Dockerfile), commands to create a dummy client, i.e. the minimal file structure required to satisfy `cargo install`.

```sh
RUN mkdir -p client/src && \
    echo '[package]\nname = "client"\nversion = "0.0.0"\n[dependencies]' > client/Cargo.toml && \
    echo 'fn main() {}' > client/src/main.rs
```

In this way, I could omit/ignore the real client.

[^1]: Essentially the same commands; I modified `build` to tag the image with version number and "latest" status.
