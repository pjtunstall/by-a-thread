# Git Recovery Playbook

When a merge went sideways or you accepted the wrong side of a conflict, the
steps below will restore the latest `main` branch (which already contains the
state refactor and the expanded test suites) and let you try again.

## 1. Re-run the merge so the conflict markers come back

If you merged and accidentally picked the wrong side, the easiest way to get the
conflicts back is to back out of the merge and try again:

```bash
# abandon any in-progress merge and reset the working tree
git merge --abort 2>/dev/null || true
git reset --hard HEAD

# grab the latest `main` from the remote and redo the merge
git fetch origin main
# replace BRANCH with the branch you want to merge into main
git checkout main
git merge origin/BRANCH
```

At this point the conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`) will be
recreated so you can choose the correct sections.

## 2. Undo the merge entirely and restart from remote `main`

If you just want to throw away the conflicted merge and start fresh from the
remote `main`, run:

```bash
git fetch origin main
git checkout main
git reset --hard origin/main
git clean -fd
```

That gives you a clean checkout of the authoritative branch with all of the
latest tests intact. Any local topic branches that you no longer need can be
removed with:

```bash
git branch -D <branch-name>
git remote prune origin
```

## 3. Verify the expanded test suite is present

Run the test listing to confirm you see the full client/server coverage:

```bash
cargo test -- --list
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
