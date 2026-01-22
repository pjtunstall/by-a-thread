#!/usr/bin/env bash

# Make this script executable by running: `chmod +x docker_script.sh`.

VERSION=$(cargo pkgid -p server | cut -d# -f2 | cut -d: -f2)

docker build \
  -t server-image:$VERSION \
  -t server-image:latest \
  .
docker run -d \
  --name server-container \
  --rm \
  -p 5000:5000/udp \
  server-image
docker logs server-container # To see the server banner with the passcode.
