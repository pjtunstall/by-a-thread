#!/usr/bin/env bash

# Make this script executable by running: `chmod +x docker_script.sh`.

docker build -t server-image .
docker run -d \
  --name server-container \
  --rm \
  -p 5000:5000/udp \
  server-image
docker logs server-container # To see the server banner with the passcode.
