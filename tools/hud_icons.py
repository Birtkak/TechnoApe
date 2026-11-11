"""The two signs of the vitals, 28 x 28 each, drawn from shapes: a mask, then its outline (o), body (#),
light (+) and shade (-). Writes a preview and prints the rows for
crates/client/src/hud.rs (CROSS, STOMACH): `python -I tools/hud_icons.py out/icons.png`."""
import sys
from PIL import Image, ImageDraw, ImageChops, ImageFilter
N, K = 28, 8  # drawn K times as large, then sampled, so curves come out even

def mask_from(draw_fn):
    im = Image.new('L', (N * K, N * K), 0)
    draw_fn(ImageDraw.Draw(im), K)
    return im.resize((N, N), Image.BOX).point(lambda v: 255 if v >= 128 else 0)

def stomach(d, k):
    # the body: round and full, bulging right and down
    d.ellipse([11 * k, 5 * k, 27 * k, 20.5 * k], fill=255)
    d.ellipse([8 * k, 10 * k, 24.5 * k, 23.5 * k], fill=255)
    # the inward curve at its left, between gullet and gut
    d.ellipse([2 * k, 7.5 * k, 10.5 * k, 15.5 * k], fill=0)
    # the gullet: short and thick, coming down into the top left
    d.line([(8 * k, 2.6 * k), (9.6 * k, 6 * k), (13 * k, 10 * k)], fill=255, width=int(5.8 * k), joint='curve')
    d.ellipse([5.1 * k, 0.2 * k, 10.9 * k, 5.4 * k], fill=255)
    # the notch between the gullet and the body's round top
    d.ellipse([11.3 * k, 2.6 * k, 14.9 * k, 6.2 * k], fill=0)
    # the gut: as thick, leaving at the lower left and turning down
    d.line([(11.5 * k, 20.3 * k), (6.5 * k, 20.5 * k), (4 * k, 23 * k), (4 * k, 25.4 * k)], fill=255, width=int(5.8 * k), joint='curve')
    d.ellipse([1.1 * k, 22.4 * k, 6.9 * k, 27.8 * k], fill=255)

def cross(d, k):
    d.rounded_rectangle([10 * k, 2 * k, 18 * k, 26 * k], radius=2 * k, fill=255)
    d.rounded_rectangle([2 * k, 10 * k, 26 * k, 18 * k], radius=2 * k, fill=255)

def tones(m, light_at):
    px = m.load()
    inside = lambda x, y: 0 <= x < N and 0 <= y < N and px[x, y] > 0
    rows = []
    for y in range(N):
        r = ''
        for x in range(N):
            if not inside(x, y):
                r += '.'
            elif not all(inside(x + dx, y + dy) for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1))):
                r += 'o'
            elif not all(inside(x + dx, y + dy) for dx, dy in ((2, 0), (0, 2), (1, 1), (2, 1), (1, 2))):
                r += '-'   # the shaded side: lower right
            elif any((x - lx) ** 2 / (rx * rx) + (y - ly) ** 2 / (ry * ry) <= 1 for lx, ly, rx, ry in light_at):
                r += '+'
            else:
                r += '#'
        rows.append(r)
    return rows

ICONS = {
    'CROSS': (tones(mask_from(cross), [(12.5, 5.5, 1.6, 2.6), (5.5, 12.5, 2.6, 1.4)]), (92, 242, 158)),
    'STOMACH': (tones(mask_from(stomach), [(19, 8.6, 4.0, 1.6), (22.8, 10.8, 1.1, 1.3), (8.2, 3.0, 1.1, 1.5)]), (255, 178, 56)),
}
out = Image.new('RGB', (2 * 300, 300), (16, 26, 39))
for k, (name, (rows, c)) in enumerate(ICONS.items()):
    col = {'o': tuple(int(v * 0.26) for v in c), '#': c, '-': tuple(int(v * 0.8) for v in c), '+': tuple(int(v * 0.6 + 102) for v in c)}
    im = Image.new('RGB', (N, N), (16, 26, 39))
    for y, r in enumerate(rows):
        for x, ch in enumerate(r):
            if ch in col:
                im.putpixel((x, y), col[ch])
    out.paste(im.resize((N * 9, N * 9), Image.NEAREST), (k * 300 + 4, 20)); out.paste(im, (k * 300 + 262, 4))
    print('const %s: [&str; %d] = [' % (name, N)); print(''.join('    "%s",\n' % r for r in rows) + '];')
out.save(sys.argv[1])
