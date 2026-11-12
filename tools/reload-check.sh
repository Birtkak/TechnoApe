#!/usr/bin/env bash
# Live reload timed end to end (ADR 0045, 0047, 0048, 0050): one off-screen run reads textures, models,
# effects and sounds from scratch copies under out/reload/ (the real data/ is never written); mid-run each
# kind's file is changed, broken, and restored. Checks by number that the change shows, within 1.5 s of the
# write, without a frame over 16.7 ms near it, and that a broken file is named and changes nothing.
# Prints one JSON verdict (tools/reload_check.py), exit 1 on a failure. About a minute; needs a release build.
# Usage: bash tools/reload-check.sh
cd "$(dirname "$0")/.."
[ -x target/release/technoape.exe ] || { echo '{"ok":false,"error":"no target/release/technoape.exe: cargo build --release"}'; exit 1; }
mkdir -p out
# one heavy job on the PC at a time: wait for out/.lock, hold it for the whole run, free it however this ends
until mkdir out/.lock 2>/dev/null; do echo "waiting for out/.lock" >&2; sleep 20; done
trap 'rmdir out/.lock' EXIT
python -I tools/reload_check.py
