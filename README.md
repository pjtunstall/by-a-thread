# FPS Project

The `main` branch now contains the latest modular client/server refactor. No additional topic branches are required to access the current code. Pulling from `main` will provide the newest changes.

## Resolving merge conflicts locally

When Git reports conflicts you will usually see **Current Change** (your local branch) and **Incoming Change** (the update you are
pulling or merging). If you want to keep the refactored client/server layout with the expanded unit tests, select the incoming
changes for files under `client/src/state.rs` and `server/src/state.rs`. Those blocks contain the additional regression tests and
state helpers. Keeping only the current change reverts to the older three-test layout.

After resolving conflicts, run `cargo test --all` to verify that both the client and server suites execute (11 client tests and 6
server tests as of the latest commit). If the test counts are lower, re-open the conflict markers and ensure the incoming blocks
were not discarded.

## Where the other branches come from

Only the `main` branch is needed to build and test the project. Any other branches you see originated
from earlier pull requests. They still exist on GitHub for historical reference, but they are not
required locally. To remove the extra branches from your machine you can run:

```bash
git checkout main
git branch -D refactor/modular-state codex/refactor-client-and-server-into-modules-d8kjn7
```

If Git reports that a branch does not exist, it simply means you never created it locally and you can
move on. Nothing else needs to be done—the remote copies will remain archived, but they no longer
affect your `main` checkout.

## Verifying you have the expanded tests

The refactored client and server state modules each carry their own unit tests. To double-check that
you kept the latest versions of those files, look for the following counts in the test output after
running `cargo test --all`:

```
running 11 tests
...
running 6 tests
```

If you only see three client tests it means the conflict resolution kept the older code. In that case
rerun the merge, pick the *incoming* blocks for `client/src/state.rs` and `server/src/state.rs`, and
test again. No work is lost as long as those incoming sections are selected.

## Git recovery quick start

If a merge left you with the old three-test layout, run `scripts/reset-to-origin-main.sh`
to sync your local `main` with the remote and then re-run `cargo test -- --list`.
For more detail, see [docs/git-recovery.md](docs/git-recovery.md).
