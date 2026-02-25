#!/bin/bash
set -euo pipefail

REPO_OWNER="pjtunstall"
REPO_NAME="by-a-thread"
DEPLOY_DIR="/home/non-root-user"
BASE_URL="https://raw.githubusercontent.com/${REPO_OWNER}/${REPO_NAME}/main"

curl -fsSL -o "${DEPLOY_DIR}/docker-compose.yaml" -H "User-Agent: curl" "${BASE_URL}/docker-compose.yaml"
curl -fsSL -o "${DEPLOY_DIR}/Caddyfile" -H "User-Agent: curl" "${BASE_URL}/Caddyfile"

docker pull pjtunstall/server-image:latest

cd "${DEPLOY_DIR}"
docker compose --env-file .env.matchmaker pull
docker compose --env-file .env.matchmaker up -d --remove-orphans

if [ -f /var/run/reboot-required ]; then
    /sbin/reboot
fi