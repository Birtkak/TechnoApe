#!/bin/bash
# Dumps and renders the in-engine look check, then builds images/sheet_look.png. Run from this folder (Git Bash).
# The seam view is yawed 60 degrees so the eye-level camera looks along the seam. The row views mark a straight
# row of blocks (blue) that crosses a seam 16 / 128 blocks from a corner; the eye stands on it 6 m before the seam.
set -e
export PATH="$HOME/Tools/mingw-dl/mingw64/bin:$PATH"
G="$HOME/Tools/Godot/4.7.2-mono/Godot_v4.7.2-stable_mono_win64/Godot_v4.7.2-stable_mono_win64_console.exe"
g++ -std=c++17 -O2 -march=native -o blocks.exe blocks.cpp
mkdir -p out images
./blocks.exe out/patch.quads 1 0.35 0.2
./blocks.exe out/seam.quads 1 1 0
./blocks.exe out/corner.quads 1 1 1
./blocks.exe out/row16.quads 1 1 1 40 16
./blocks.exe out/row128.quads 1 1 1 40 128
shot() { shot1 "$@" || shot1 "$@"; }  # a run now and then saves nothing when other Godot jobs are busy
shot1() { (cd godot && timeout 90 "$G" --path . --resolution 640x360 --position -30000,-30000 --script res://look.gd -- "$@" | grep -E "saved|ERROR" ); }
for n in patch seam corner row16 row128; do
    yaw=0; [ $n = seam ] && yaw=60
    top="alt=7 pitch=72"; [ ${n:0:3} = row ] && top="alt=12 pitch=60"
    shot in=../out/$n.quads out=../images/look_$n yaw=$yaw
    shot in=../out/$n.quads out=../images/top_$n yaw=$yaw $top
done
python -I - <<'EOF'
from PIL import Image, ImageDraw
rows, cols = ["patch", "seam", "corner", "row16", "row128"], ["look", "top"]
name = {"patch": "patch", "seam": "seam middle", "corner": "cube corner",
        "row16": "row crossing a seam 16 blocks from a corner", "row128": "row crossing a seam 128 blocks from a corner"}
W, H, T = 640, 360, 24
s = Image.new("RGB", (4 * W, len(rows) * (H + T)), "white")
d = ImageDraw.Draw(s)
for r, n in enumerate(rows):
    for c, (v, k) in enumerate((v, k) for v in cols for k in ("flat", "tex")):
        s.paste(Image.open(f"images/{v}_{n}_{k}.png").convert("RGB"), (c * W, r * (H + T) + T))
        mark = ", seam orange / corner pink" if k == "flat" and n != "patch" else ""
        up = "from above" if v == "top" else "eye level"
        d.text((c * W + 8, r * (H + T) + 8), f"{name[n]}: {up}, {'flat colours' if k == 'flat' else 'textured'}{mark}", fill="black")
s.save("images/sheet_look.png")
EOF
