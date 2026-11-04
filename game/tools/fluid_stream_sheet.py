"""Before/after sheet of consecutive full-size frames of a falling stream (ADR 0138): a crop of each fluid_full shot,
one row per run. python tools/fluid_stream_sheet.py out/before out/after fall [x0,y0,x1,y1] [dest.png]"""
import glob
import sys
from PIL import Image, ImageDraw

before, after, scene = sys.argv[1:4]
box = (540, 120, 760, 700)
dest = "%s/fluid_%s_stream.png" % (after, scene)
for a in sys.argv[4:]:
    if a.count(",") == 3:
        box = tuple(int(v) for v in a.split(","))
    else:
        dest = a
rows = []
for d in (before, after):
    shots = sorted(glob.glob("%s/fluid_%s_full_*.png" % (d, scene)), key=lambda p: float(p.rsplit("_", 1)[1][:-4]))
    rows.append([Image.open(p).convert("RGB").crop(box) for p in shots])
w, h = box[2] - box[0], box[3] - box[1]
n = max(len(r) for r in rows)
img = Image.new("RGB", (60 + n * (w + 2), 20 + 2 * (h + 2)), (20, 20, 20))
dr = ImageDraw.Draw(img)
for r, (label, fs) in enumerate(zip(("before", "after"), rows)):
    dr.text((6, 20 + r * (h + 2) + h // 2), label, fill=(230, 230, 230))
    for i, f in enumerate(fs):
        img.paste(f, (60 + i * (w + 2), 20 + r * (h + 2)))
dr.text((60, 4), "consecutive frames, ~17 ms apart (sim ticks every 33 ms)", fill=(230, 230, 230))
img.save(dest)
print(dest)
