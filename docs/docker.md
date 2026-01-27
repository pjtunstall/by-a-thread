# Docker

- [To run on locally](#to-run-locally)
- [To run on Hetzner](#to-run-on-hetzner)

## To run locally

Assuming you've installed [Docker](https://www.docker.com/)--and started it with `sudo systemctl start docker` if need be--you can run the server on Docker and the client directly on your host machine. Build the server image from the project root (where `Dockerfile` lives):

```sh
# Extract the version number to tag the image with.
VERSION=$(cargo pkgid -p server | cut -d# -f2 | cut -d: -f2)

# Tag the image with the version number and as "latest".
docker build \
  -t server-image:$VERSION \
  -t server-image:latest \
  .
```

Then run the server:

```sh
docker run -d \
  --name server-container \
  --rm \
  -e IP=127.0.0.1 \
  -p 5000:5000/udp \
  server-image
```

Tell Docker to log output so far, so that we can the server banner with the passcode:

```sh
docker logs server-container
```

Then run the client as usual.

(A container stops when its main process exits. In this case, the main process is the server. The server will exit shortly after the last client leaves. In case you want to stop it immediately, `stop server-container`.)

## To run on Hetzner

This section assumes you have a Hetzner VPS, suitably configured.

From your local machine, push the latest image of the server to your VPS:

```sh
docker save server-image | gzip | ssh hetzner 'gunzip | docker load'
```

Then SSH into the VPS:

```sh
ssh hetzner
```

And run the server container:

```sh
# Set IP to the IP address of the VPS.
docker run -d \
 --name server-container \
 --rm \
 -e IP=$(curl -s http://169.254.169.254/hetzner/v1/metadata/public-ipv4) \
 -p 5000:5000/udp \
 server-image
```

And run the client locally, as usual with `cargo run --release -p client`.

As before, stop the container with `docker stop server-container`, or just let it stop by itself when all players have left.
