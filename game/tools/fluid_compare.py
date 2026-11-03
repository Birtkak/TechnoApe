"""Before/after contact sheet of a fluid_film scene (ADR 0138): a row of frames from each run's strip.
   python tools/fluid_compare.py out/ff2_before out/ff2_after pit_lake [frames=0,3,6,9,14,22] [out.png]
The strips are FrameSheet grids (480 x 270 thumbnails, 2 px gaps, 4 columns, one every fluid_every s)."""
import sys
from PIL import Image, ImageDraw

TW, TH, GAP, COLS = 480, 270, 2, 4


def frames(path, picks):
    sheet = Image.open(path).convert("RGB")
    n = (sheet.width // (TW + GAP)) * (sheet.height // (TH + GAP))
    out = []
    for k in picks:
        k = min(k, n - 1)
        x, y = (k % COLS) * (TW + GAP), (k // COLS) * (TH + GAP)
        out.append(sheet.crop((x, y, x + TW, y + TH)))
    return out


def main():
    before, after, scene = sys.argv[1:4]
    picks = [0, 3, 6, 9, 14, 22]
    dest = "%s/fluid_%s_compare.png" % (after, scene)
    for a in sys.argv[4:]:
        if a.startswith("frames="):
            picks = [int(v) for v in a[7:].split(",")]
        else:
            dest = a
    rows = [("before", frames("%s/fluid_%s_strip.png" % (before, scene), picks)),
            ("after", frames("%s/fluid_%s_strip.png" % (after, scene), picks))]
    W, H = 60 + len(picks) * (TW + GAP), 24 + 2 * (TH + GAP)
    img = Image.new("RGB", (W, H), (20, 20, 20))
    d = ImageDraw.Draw(img)
    for i, k in enumerate(picks):
        d.text((60 + i * (TW + GAP) + 4, 6), "frame %d" % k, fill=(230, 230, 230))
    for r, (label, fs) in enumerate(rows):
        d.text((6, 24 + r * (TH + GAP) + TH // 2), label, fill=(230, 230, 230))
        for i, f in enumerate(fs):
            img.paste(f, (60 + i * (TW + GAP), 24 + r * (TH + GAP)))
    img.save(dest)
    print(dest)


main()
