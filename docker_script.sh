#!/usr/bin/env bash

# To make this script executable, run `chmod +x docker_script.sh`.

# Extract the version number to tag the image with.
VERSION=$(cargo pkgid -p server | cut -d# -f2 | cut -d: -f2)

# Tag the image with the version number and as "latest".
docker build \
  -t server-image:$VERSION \
  -t server-image:latest \
  .

# Run the container in detached mode (i.e. in the background).
# Remove the container when it stops.
docker run -d \
  --name server-container \
  --rm \
  -p 5000:5000/udp \
  server-image

# Log output so far, so that we can the server banner with the passcode.
docker logs server-container

# NOTE: A container stops when its main process exits. The server will exit
# shortly after the last client leaves. In case you want to stop it immediately,
# run `stop server-container`.
