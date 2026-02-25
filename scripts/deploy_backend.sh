#!/bin/bash
set -euo pipefail

REPO_OWNER="pjtunstall"
REPO_NAME="by-a-thread"
DEPLOY_DIR="/home/non-root-user"

curl -fsSLo "${DEPLOY_DIR}/docker-compose.yaml" \
  "https://raw.githubusercontent.com/${REPO_OWNER}/${REPO_NAME}/main/docker-compose.yaml"

curl -fsSLo "${DEPLOY_DIR}/Caddyfile" \
  "https://raw.githubusercontent.com/${REPO_OWNER}/${REPO_NAME}/main/Caddyfile"

docker pull pjtunstall/server-image:latest

cd "${DEPLOY_DIR}"
docker compose --env-file .env.matchmaker pull
docker compose --env-file .env.matchmaker up -d --remove-orphans

if [ -f /var/run/reboot-required ]; then
    /sbin/reboot
fi