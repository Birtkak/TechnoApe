#!/usr/bin/env bash
# Make a fresh agent worktree runnable in one step (see .claude/skills/overseer/SKILL.md).
# Run from inside the worktree: bash tools/agent_worktree_setup.sh [--native]
# 1. Moves the branch to local main when it has no commits of its own (worktrees are cut
#    from origin/main, which lags local main while nothing is pushed).
# 2. Copies the gitignored native DLLs and gdUnit4's bin folder from the main checkout.
# 3. With --native: gives the worktree a private godot-cpp copy (parallel builds race on its gen/ folder)
#    and prints the GODOT_CPP_PATH to build with.
# 4. Imports the project so new class_names and textures are known.
set -euo pipefail

wt="$(git rev-parse --show-toplevel)"
main_wt="$(git worktree list --porcelain | awk '/^worktree /{print substr($0, 10); exit}')"
name="$(basename "$wt")"

if [ "$wt" = "$main_wt" ]; then
	echo "run this inside an agent worktree, not the main checkout" >&2
	exit 1
fi

if [ -z "$(git rev-list main..HEAD)" ] && [ "$(git rev-parse HEAD)" != "$(git rev-parse main)" ]; then
	git reset -q --hard main
	echo "base: moved to main $(git rev-parse --short HEAD)"
else
	echo "base: $(git rev-parse --short HEAD) ($(git rev-list --count main..HEAD) own commits, $(git rev-list --count HEAD..main) behind main)"
fi

for d in game/addons/technosim/bin game/addons/zylann.voxel/bin game/addons/gdUnit4/bin; do
	mkdir -p "$wt/$d"
	cp -r "$main_wt/$d/." "$wt/$d/"
done
echo "bins: copied from $main_wt"

if [ "${1:-}" = "--native" ]; then
	cpp="$HOME/Tools/godot-cpp-wt/$name"
	if [ ! -d "$cpp" ]; then
		mkdir -p "$(dirname "$cpp")"
		cp -r "$HOME/Tools/godot-cpp" "$cpp"
	fi
	echo "godot-cpp: export GODOT_CPP_PATH=\"$cpp\""
fi

g="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
timeout 300 "$g" --headless --path "$wt/game" --import > /dev/null 2>&1 || echo "import: exited $? (check by running it yourself)"
echo "import: done"
