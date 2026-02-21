Old section of Makefile to build, deploy, and run a single game server.

```sh
# --- Build Docker image of game server (prerequisite for server target) ---

#

# Prerequisites: Docker (https://docs.docker.com/engine/install)

#

$(DOCKER_SENTINEL): $(SERVER_BIN) server/Dockerfile | check-docker

mkdir -p $(DIST)

VERSION=$$(cargo pkgid -p server | cut -d# -f2 | cut -d: -f2); \

docker build -f server/Dockerfile -t server-image:$$VERSION -t server-image:latest .

touch $(DOCKER_SENTINEL)



# --- Update game server on VPS ---

#

# Prerequisites: VPS running; SSH access as 'hetzner'; docker in PATH on VPS

#

deploy-hetzner: $(DOCKER_SENTINEL) | check-deploy

docker save server-image | gzip | ssh hetzner 'gunzip | docker load'

ssh hetzner 'docker stop server-container 2>/dev/null; docker rm server-container 2>/dev/null; docker run -d --name server-container --rm --read-only --cap-drop ALL --security-opt no-new-privileges --cpus 0.4 --pids-limit 256 -e IP=$$(curl -s http://169.254.169.254/hetzner/v1/metadata/public-ipv4) -p 5000:5000/udp server-image'

ssh hetzner 'docker logs server-container'

run-hetzner: | check-deploy

ssh hetzner 'docker stop server-container 2>/dev/null; docker rm server-container 2>/dev/null; docker run -d --name server-container --rm --read-only --cap-drop ALL --security-opt no-new-privileges --cpus 0.4 --pids-limit 256 -e IP=$$(curl -s http://169.254.169.254/hetzner/v1/metadata/public-ipv4) -p 5000:5000/udp server-image'

ssh hetzner 'docker logs server-container'
```
