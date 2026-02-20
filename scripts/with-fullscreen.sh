#!/usr/bin/env bash
# This script wraps a command: it patches client/src/main.rs, adding
# `fullscreen: true` to the window config. (The implicit default, `fullscreen:
# false`, is more convenient for development, but `fullscreen: true` looks better in
# production.) Next the script runs the command. Finally, it restores main.rs to
# its original state on exit (success or failure of the command). We use a
# script rather than Make inline commands because a trap must span the whole
# command to restore on failure; Make recipe lines run in separate subshells.
# Called from the Makefile; not intended to be run alone. It should be run from
# the workspace root.
set -e

MAIN_RS=client/src/main.rs
cp "$MAIN_RS" "$MAIN_RS.bak"
trap "mv '$MAIN_RS.bak' '$MAIN_RS'" EXIT

if grep -q 'fullscreen: false,' "$MAIN_RS"; then
    sed 's|fullscreen: false,|fullscreen: true,|' "$MAIN_RS" > "$MAIN_RS.tmp" && mv "$MAIN_RS.tmp" "$MAIN_RS"
fi

"$@"
