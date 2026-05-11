#!/bin/bash
# Before this script runs, replace YOUR_GITHUB_TOKEN with an actual GitHub PAT,
# finegrained, repo-scoped, contents: Read and write, and Metadata: Read-only.
set -euo pipefail

curl -fsS -X POST \
  -H "Authorization: token YOUR_GITHUB_TOKEN" \
  -H "Accept: application/vnd.github.v3+json" \
  https://api.github.com/repos/pjtunstall/by-a-thread/dispatches \
  -d '{"event_type":"deploy_client"}'
  