"""Tide filmstrips (ADR 0064): one frame of run=stand per world time, laid out in a sheet with the time under it.

python -I prototypes/tides/film.py <technoape.exe> <out.png> <view args> -- t1 t2 ...
"""
import subprocess, sys, os
from PIL import Image, ImageDraw

exe, out = sys.argv[1], sys.argv[2]
k = sys.argv.index("--")
view, times = sys.argv[3:k], [float(t) for t in sys.argv[k + 1:]]
tmp = out + ".frames"
os.makedirs(tmp, exist_ok=True)
frames = []
for n, t in enumerate(times):
    p = os.path.join(tmp, f"{n:02}.png")
    subprocess.run([exe, "run=stand", f"time={t}", "quick=1", "capture_ms=60000", "capture_w=480", f"sheet={p}", *view], check=True, capture_output=True)
    im = Image.open(p).convert("RGB")
    frames.append(im.crop((0, 0, 480, 270)))
w, h = frames[0].size
cols = 8 if len(frames) > 8 else len(frames)
rows = (len(frames) + cols - 1) // cols
sheet = Image.new("RGB", (cols * w, rows * (h + 18)), "white")
d = ImageDraw.Draw(sheet)
for n, (im, t) in enumerate(zip(frames, times)):
    x, y = n % cols * w, n // cols * (h + 18)
    sheet.paste(im, (x, y))
    d.text((x + 4, y + h + 3), f"t={t:.0f} s (day {t / 1800:.2f})", fill="black")
sheet.save(out)
print(out, len(frames), "frames")
