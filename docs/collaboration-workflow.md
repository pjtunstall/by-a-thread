# Collaboration Workflow

This project uses a simple mainline flow so that both of us stay in sync without
needing to resolve the same conflicts twice. The steps below describe the
expected routine for each side.

## What I will do before every pull request

1. `git fetch origin`
2. `git checkout main`
3. `git reset --hard origin/main`
4. `git clean -fd`
5. `git checkout -b codex/<short-description>`
6. Make the change, commit, push, and open the PR.
7. After the PR is merged, delete the feature branch locally and on GitHub.

Because every PR branch starts from the freshly fetched `origin/main`, GitHub
never needs an "Update branch" merge. If the tip of `main` moves while the PR is
open, I'll rebase the branch and force-push so that it stays conflict-free.

## What you need to do to test a merged change

1. `git fetch origin main`
2. `git checkout main`
3. `git reset --hard origin/main`
4. `git clean -fd`
5. Run your usual test commands (e.g. `cargo test --all`).

Those four commands reset your working tree to match the exact merge commit that
GitHub produced. There's no need to click "Update branch"—that button only
merges `main` _into_ an older feature branch, and we already reset the branch to
the latest `main` before opening the PR.

### Why the "Update branch" button sticks around

GitHub shows the button whenever a PR branch was created before the most recent
`main` commit. Because I create a fresh branch at the start of every PR, the
button is safe to ignore: we deliberately avoid pressing it so that GitHub does
not synthesize extra merge commits. Instead, wait for the PR to merge and then
run the four sync commands above to grab the merged result.

If you ever need to review a branch _before_ it merges, fetch it directly (see
"Diagnosing a mismatched tree" below) rather than relying on "Update branch".

## Cleaning up stale remote branches

If a feature branch lingers on the remote after a merge, you can delete it with
one command:

```bash
git push origin --delete <branch-name>
```

(You can also use the "Delete branch" button that GitHub shows once a PR is
merged.) Removing stale branches keeps the branch list tidy and prevents
confusion about which branch is the current work in progress.

## Diagnosing a mismatched tree

If you ever run the sync commands above and still see unexpected files or build
failures, double-check which commit you are on with:

```bash
git status -sb
git log --oneline --decorate -5
git rev-parse HEAD
```

Then compare the `HEAD` hash with the merge SHA that GitHub displays on the PR
page (or in the "Latest commit" badge). If those hashes differ, your local tree
is still pointed at an older commit. Repeat the reset commands or let me know so
I can rebase the feature branch and push an updated commit for you to pull.

If you want to inspect the exact code I have before a PR is merged, you can
fetch the in-progress branch directly:

```bash
git fetch origin <branch-name>
git checkout <branch-name>
```

That checkout gives you the identical tree I am testing against. Once the PR is
merged you can return to `main` with `git checkout main && git pull --ff-only`.

Following this workflow ensures we both share the same history and that new PRs
arrive without conflicting merges.
