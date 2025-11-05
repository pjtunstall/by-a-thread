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

See also: [Collaboration Workflow](./collaboration-workflow.md) for the day-to-day
routine that keeps both sides in sync.
