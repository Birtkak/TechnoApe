#!/usr/bin/env bash
# Builds for the developer to double-click (ADR 0023): `bash tools/playtest.sh build` copies a release build to
# build/TechnoApe.exe, the newest one; `bash tools/playtest.sh` (or `playtest`) to playtest/TechnoApe.exe, the one
# handed over to play-test, which changes only then. Started without arguments either opens the title menu with
# the repository's saves/ and bugs/. target/ stays Claude's own.
set -euo pipefail
cd "$(dirname "$0")/.."
to="${1:-playtest}"
[[ "$to" == build || "$to" == playtest ]] || { echo "usage: tools/playtest.sh [build|playtest]" >&2; exit 2; }
export PATH="$HOME/Tools/rust/cargo/bin:$PATH" CARGO_HOME="$HOME/Tools/rust/cargo" RUSTUP_HOME="$HOME/Tools/rust/rustup"
cargo build --release
mkdir -p "$to"
cp target/release/technoape.exe "$to/TechnoApe.exe" || { echo "$to/TechnoApe.exe is in use: close the game first" >&2; exit 1; }
echo "$to/TechnoApe.exe ($(git rev-parse --short HEAD)$(git diff --quiet HEAD -- || echo '+changes'))"
