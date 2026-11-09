#!/bin/bash
# Renders images/sheet_relaxed.png: the equi-angular grid against two relaxed ones (smooth.cpp), flat look.
# Rows: equi-angular, relaxed with the corners kept equi-angular (L = 128 blocks), fully relaxed.
# Columns: row 128 blocks from a corner at eye level, row 16 blocks from a corner at eye level, both rows from
# above, the cube corner from above. Needs out/harmonic.tab (./smooth.exe 192). Run from this folder (Git Bash).
set -e
export PATH="$HOME/Tools/mingw-dl/mingw64/bin:$PATH"
G="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
g++ -std=c++17 -O2 -march=native -o blocks.exe blocks.cpp
mkdir -p out images
shot() { shot1 "$@" || shot1 "$@"; }  # a run now and then saves nothing when other Godot jobs are busy
shot1() { (cd godot && timeout 90 "$G" --path . --resolution 640x360 --position -30000,-30000 --script res://look.gd -- "$@" | grep -E "saved|ERROR" ); }
for v in "ea 0 0" "fade 1 128" "full 1 0"; do
    set -- $v
    ./blocks.exe out/r_$1_row128.quads 1 1 1 40 128 $2 $3
    ./blocks.exe out/r_$1_row16.quads 1 1 1 40 16 $2 $3
    ./blocks.exe out/r_$1_corner.quads 1 1 -1 40 -1 $2 $3
    shot in=../out/r_$1_row128.quads out=../out/r_$1_row128_look
    shot in=../out/r_$1_row16.quads out=../out/r_$1_row16_look
    shot in=../out/r_$1_row16.quads out=../out/r_$1_row16_top alt=12 pitch=60
    shot in=../out/r_$1_corner.quads out=../out/r_$1_corner_top alt=10 pitch=89
done
python -I - <<'EOF'
from PIL import Image, ImageDraw
rows = [("ea", "equi-angular (now)"), ("fade", "relaxed, corners kept (L = 128 blocks)"), ("full", "fully relaxed (conformal)")]
cols = [("row128_look", "row 128 blocks from a corner, eye level"), ("row16_look", "row 16 blocks from a corner, eye level"),
        ("row16_top", "row 16 blocks from a corner, from above"), ("corner_top", "cube corner, straight down")]
W, H, T = 640, 360, 24
s = Image.new("RGB", (len(cols) * W, len(rows) * (H + T)), "white")
d = ImageDraw.Draw(s)
for r, (v, vn) in enumerate(rows):
    for c, (k, kn) in enumerate(cols):
        s.paste(Image.open(f"out/r_{v}_{k}_flat.png").convert("RGB"), (c * W, r * (H + T) + T))
        d.text((c * W + 8, r * (H + T) + 8), f"{vn}: {kn}", fill="black")
s.save("images/sheet_relaxed.png")
EOF
