#!/usr/bin/env bash
set -euo pipefail

if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "This script must be run inside a git repository." >&2
  exit 1
fi

current_branch=$(git rev-parse --abbrev-ref HEAD)

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "You have uncommitted changes. Stash or commit them before running this script." >&2
  exit 1
fi

echo "Fetching latest main from origin..."
git fetch origin main

echo "Checking out main..."
git checkout main

echo "Resetting local main to origin/main..."
git reset --hard origin/main

echo "Cleaning untracked files..."
git clean -fd

echo "Pruning remote-tracking branches that no longer exist..."
git remote prune origin

echo "Local main now matches origin/main."

echo "Run 'cargo test -- --list' to confirm the expanded test suite is present."

echo "If you were previously on a different branch ($current_branch), you can re-create it from the updated main with:\n  git switch -c $current_branch"
