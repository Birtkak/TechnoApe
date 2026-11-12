"""A year in 12 frames (docs/design/seasonal-looks.md's check): one view at noon of every 4.5th day, as a sheet.

  python -I tools/year_sheet.py out/year-spawn.png at=0.0347,0.8675,0.4975 eye=6 pitch=-25 yaw=0

The arguments after the sheet's name are run=stand's; time= is set per frame.
"""
import pathlib, subprocess, sys
from PIL import Image, ImageDraw

DAY, YEAR, N = 1800, 54 * 1800, 12
out, args = pathlib.Path(sys.argv[1]), sys.argv[2:]
exe = pathlib.Path(__file__).resolve().parent.parent / "target/release/technoape.exe"
frames = []
for k in range(N):
    # whole days from the views' noon (225 s), so every frame has the same hour
    t = 225 + round(k * YEAR / N / DAY) * DAY
    png = out.with_name(f"{out.stem}-{k:02}.png")
    subprocess.run([exe, "run=stand", *args, f"time={t}", f"shot={png}", "save=0"], check=True, capture_output=True, timeout=180)
    im = Image.open(png).convert("RGB")
    im.thumbnail((640, 360))
    ImageDraw.Draw(im).text((8, 6), f"day {t // DAY} ({['spring', 'summer', 'autumn', 'winter'][k * 4 // N]})", fill=(255, 255, 255))
    frames.append(im)
    png.unlink()
w, h = frames[0].size
sheet = Image.new("RGB", (w * 4, h * 3))
for k, im in enumerate(frames):
    sheet.paste(im, (k % 4 * w, k // 4 * h))
sheet.save(out)
print(out)
