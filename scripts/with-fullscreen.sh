#!/usr/bin/env bash
# Wraps a command: patches main.rs for fullscreen, runs the command, restores on exit (success or failure).
# A script rather than Make inline commands because a trap must span the whole command to restore on failure; Make recipe lines run in separate subshells.
# Called from the Makefile; not intended to be run alone. Run from workspace root.
set -e

MAIN_RS=client/src/main.rs
cp "$MAIN_RS" "$MAIN_RS.bak"
trap "mv '$MAIN_RS.bak' '$MAIN_RS'" EXIT

if grep -q 'fullscreen: false,' "$MAIN_RS"; then
    sed 's|fullscreen: false,|fullscreen: true,|' "$MAIN_RS" > "$MAIN_RS.tmp" && mv "$MAIN_RS.tmp" "$MAIN_RS"
fi

"$@"
