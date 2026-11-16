#!/usr/bin/env python3
"""A contact sheet of painted-box models seen from the front (+z, as the hand holds them), to read without the game:

    python -I tools/model_sheet.py out/sheet.png "copper pick" flint ...

Each model's boxes are drawn back to front by their +z face only, at 8 screen pixels a model pixel, on grass green.
shortcut: no turn, no side or top faces; a model with turned boxes shows them unturned. The game's own shot is the
real check (`run=stand model=name`)."""
import sys
import tomllib
from pathlib import Path

from PIL import Image, ImageDraw

MODELS = Path(__file__).resolve().parent.parent / "data" / "models"
S, CELL = 8, 32


def front(name):
    row = tomllib.loads((MODELS / f"{name}.toml").read_text(encoding="utf-8"))
    png = Image.open(MODELS / f"{name}.png").convert("RGBA")
    out = Image.new("RGBA", (CELL * S, CELL * S), (86, 125, 70, 255))
    boxes = [b for p in row["part"] for b in p.get("boxes", [])]
    for b in sorted(boxes, key=lambda b: b["at"][2] + b["size"][2]):
        (x, y, _), (sx, sy, sz), (u, v) = b["at"], b["size"], b["uv"]
        if sx == 0 or sy == 0:
            continue
        face = png.crop((u + sz, v + sz, u + sz + sx, v + sz + sy)).resize((sx * S, sy * S), Image.NEAREST)
        out.alpha_composite(face, (round(x * S), round((CELL - y - sy) * S)))
    ImageDraw.Draw(out).text((4, 2), name, fill=(255, 255, 255, 255))
    return out


if __name__ == "__main__":
    names = sys.argv[2:]
    cols = min(6, len(names))
    sheet = Image.new("RGBA", (cols * CELL * S, -(-len(names) // cols) * CELL * S))
    for k, n in enumerate(names):
        sheet.paste(front(n), (k % cols * CELL * S, k // cols * CELL * S))
    sheet.save(sys.argv[1])
    print(sys.argv[1])
