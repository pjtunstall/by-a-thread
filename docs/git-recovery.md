# Git Recovery Playbook

These are simple scripts and references to help recover in case something goes
wrong with this repo.

## Resetting your tree to match origin/main

```bash
#!/bin/bash
set -euxo pipefail

git fetch origin main
git checkout main
git reset --hard origin/main
git clean -fd
```

## Getting rid of branches you don't need

```bash
#!/bin/bash
for branch in $(git branch -r | grep <PATTERN-TO-DELETE>); do
    git push origin --delete "${branch#origin/}"
done
```

To remove a single branch, use:

```bash
git push origin --delete <branch-name>
```

You should see eleven client tests and six server tests (plus the shared crate’s
docs tests). If you only see three client tests, you are still on the old code
and should repeat step 2.

## 4. Automate the reset

Once you are confident you want the canonical `main`, you can run the helper
script added in `scripts/reset-to-origin-main.sh` to perform step 2 for you.

## Why the “Update branch” button appears

GitHub shows “Update branch” when a PR branch is behind the target branch. It
does not mean the code lives on some other branch—it just needs to be rebased or
merged with the current `main`. By resetting to `origin/main` you’re guaranteed
to have the same content that was merged.

## Why not?

### Deleting a single stale remote branch

If you only need to remove one remote branch (for example,
`codex/evaluate-macroquad-ui`), run:

```bash
git push origin --delete codex/evaluate-macroquad-ui
```

GitHub will immediately drop the branch from the list of active branches. Any
open pull requests that referenced it will automatically close.

### Deleting all remote branches except `main`

When you want to prune every remote branch except `main`, the following command
will iterate over the remote list and delete them one by one:

```bash
git branch -r | grep origin | grep -v "origin/main" | sed 's/origin\///' | xargs -I {} git push origin --delete {}
```
