#!/bin/bash
# Before this script runs,

# 1.Make sure that Docker is installed and configured to
# restart after a reboot:
# sudo systemctl enable docker.service
# sudo systemctl enable containerd.service

# 2. Run `sudo visudo` and add
# `non-root-user ALL=(ALL) NOPASSWD: /usr/sbin/reboot`
# to the end of the sudoers file.

# 3. Run `sudo systemctl edit docker.service` and make sure that the following
#    lines are present and uncommented so that Docker only starts after the
# network is online and the time is set:
# After=network-online.target nss-lookup.target docker.socket containerd.service time-set.target
# Wants=network-online.target containerd.service

set -euo pipefail

REPO_OWNER="pjtunstall"
REPO_NAME="by-a-thread"
DEPLOY_DIR="${DEPLOY_DIR:-$HOME}"
BASE_URL="https://raw.githubusercontent.com/${REPO_OWNER}/${REPO_NAME}/main"

curl -fsSL -o "${DEPLOY_DIR}/docker-compose.yaml" -H "User-Agent: curl" "${BASE_URL}/docker-compose.yaml"
curl -fsSL -o "${DEPLOY_DIR}/Caddyfile" -H "User-Agent: curl" "${BASE_URL}/Caddyfile"

docker pull pjtunstall/server-image:latest

cd "${DEPLOY_DIR}"
docker compose --env-file .env.matchmaker pull
docker compose --env-file .env.matchmaker up -d --remove-orphans

docker system prune -f

if [ -f /var/run/reboot-required ]; then
    sudo /usr/sbin/reboot
fi
