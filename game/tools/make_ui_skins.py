"""Generate the UI's nine-slice skins (ADR 0129: the "workshop" skin) into game/assets/ui/<name>.png.

UiStyle.skin reads them: corners are a third of the image, drawn 3 screen pixels per image pixel, edges and
centre tiled, so every ring here is uniform along its length and every centre flat. Lit from the top-left:
top and left edges take the light, bottom and right the shade, shades cool and lights warm. Dark slate panels
in a riveted bronze frame for menus and the HUD; the inventory and recipe browser stay light (their labels are
dark) in the same frame; buttons are raised steel plates with the gold accent on hover. A hand-painted
game/art/ui/<name>.png wins over the drawn one (game/art/README.md).
book_button has no skin on purpose: one image would hide its hover colour.

Run: python game/tools/make_ui_skins.py [sheet.png]   then godot --headless --path game --import
"""
import os
import sys

from PIL import Image


def c(h, a=255):
    return tuple(int(h[i:i + 2], 16) for i in (1, 3, 5)) + (a,)


IRON_D = c("#16171d")
BRONZE_D, BRONZE, BRONZE_L, RIVET = c("#5e3f1a"), c("#a8793a"), c("#dcae62"), c("#fbe3a2")
GOLD_D, GOLD, GOLD_L = c("#a87a22"), c("#facc4c"), c("#fff0b0")
STEEL_D, STEEL, STEEL_L = c("#1f2229"), c("#2e323c"), c("#6a7284")


def mid(a, b):
    return tuple((x + y) // 2 for x, y in zip(a, b))


def skin(n, rings, fill, rivets=None, cut=True):
    """An n x n image: rings[k] = (top-left colour, bottom-right colour) for the k-th ring from the edge,
    fill inside them; rivets = (ring, colour) puts a rivet head on that ring's four corners; cut leaves the
    outermost corner pixels clear (a bevelled corner)."""
    img = Image.new("RGBA", (n, n), fill)
    px = img.load()
    for k, (tl, br) in enumerate(rings):
        for i in range(k, n - k):
            for x, y, top_left in ((i, k, True), (k, i, True), (i, n - 1 - k, False), (n - 1 - k, i, False)):
                px[x, y] = tl if top_left else br
        px[n - 1 - k, k] = mid(tl, br)
        px[k, n - 1 - k] = mid(tl, br)
    if rivets:
        k, col = rivets
        for x, y in ((k, k), (n - 1 - k, k), (k, n - 1 - k), (n - 1 - k, n - 1 - k)):
            px[x, y] = col
    if cut:
        for x, y in ((0, 0), (n - 1, 0), (0, n - 1), (n - 1, n - 1)):
            px[x, y] = (0, 0, 0, 0)
    return img


SLATE = c("#181a20", 240)
FRAME = [(IRON_D, IRON_D), (BRONZE_L, BRONZE_D), (IRON_D, IRON_D)]
LIGHT = c("#c8c4bb")

SKINS = {
    # menus (title, pause, settings, playtest menus) and the quest book
    "menu_panel": skin(12, FRAME + [(c("#2a2d37", 240), c("#101116", 240))], SLATE, (1, RIVET)),
    "book": skin(12, FRAME + [(c("#2a2d37", 248), c("#101116", 248))], c("#17181e", 248), (1, RIVET)),
    "book_pane": skin(6, [(c("#0c0d11"), c("#33363f"))], c("#20222a"), cut=False),
    "tab_pane": skin(9, [(IRON_D, IRON_D), (c("#2a2d37"), c("#101116"))], c("#1b1d23"), cut=False),
    # buttons and tabs: raised steel plates; hover rims gold, pressed sinks into warm bronze
    "button": skin(12, [(IRON_D, IRON_D), (STEEL_L, STEEL_D), (c("#3a3f4b"), c("#262930"))], STEEL),
    "button_hover": skin(12, [(GOLD_D, GOLD_D), (GOLD_L, GOLD), (c("#4a5060"), c("#2c3038"))], c("#383d49")),
    "button_pressed": skin(12, [(IRON_D, IRON_D), (c("#3a2a12"), BRONZE_L), (c("#5a4520"), c("#8a6a34"))], c("#6e5630")),
    "button_disabled": skin(12, [(c("#22242b"), c("#22242b")), (c("#1d1f25"), c("#1d1f25"))], c("#1b1c21")),
    "tab": skin(9, [(IRON_D, IRON_D), (STEEL_L, STEEL_D)], STEEL),
    "tab_selected": skin(9, [(GOLD_D, GOLD_D), (GOLD_L, BRONZE_D)], c("#6e5630")),
    # the HUD: see-through slate, bronze-rimmed; the hotbar's cells recessed, the selected one framed in gold
    "hud_box": skin(9, [(c("#0d0e12", 220), c("#0d0e12", 220)), (c("#8a6430", 200), c("#4a3418", 200))], c("#121318", 190)),
    "tooltip": skin(9, [(IRON_D, IRON_D), (BRONZE_L, BRONZE_D)], c("#15161c", 245)),
    "hotbar": skin(12, [(c("#0d0e12", 230), c("#0d0e12", 230)), (c("#8a6430", 220), c("#4a3418", 220)),
                        (c("#22252d", 170), c("#0a0b0e", 170))], c("#121318", 160), (1, c("#e8c880", 230))),
    "hotbar_slot": skin(9, [(c("#07080a", 200), c("#3c404c", 200))], c("#000000", 105), cut=False),
    "hotbar_selected": skin(9, [(IRON_D, IRON_D), (GOLD_L, GOLD), (GOLD, GOLD_D)], (0, 0, 0, 0)),
    # the inventory and the recipe browser: light, framed in bronze; slots sunk in, cool shade and warm light
    "inventory": skin(12, FRAME + [(c("#eeebe3"), c("#8e8a82"))], LIGHT, (1, RIVET)),
    "slot": skin(6, [(c("#34343c"), c("#f2eee2"))], c("#8a8986"), cut=False),
}


def main():
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "assets", "ui")
    os.makedirs(out, exist_ok=True)
    art = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "art", "ui")
    for name in SKINS:
        own = os.path.join(art, name + ".png")
        if os.path.exists(own):
            SKINS[name] = Image.open(own).convert("RGBA")
            print("art override", name)
        SKINS[name].save(os.path.join(out, name + ".png"))
    print("wrote", len(SKINS), "skins to", os.path.abspath(out))
    if len(sys.argv) > 1:   # a contact sheet: each skin as UiStyle draws it (3x) at 120 x 72
        w, h = 132, 84
        sheet = Image.new("RGBA", (w * 6, h * ((len(SKINS) + 5) // 6)), c("#5a7aa8"))
        for i, (name, img) in enumerate(SKINS.items()):
            n = img.width
            k = n // 3
            big = img.resize((n * 3, n * 3), Image.NEAREST)
            tile = Image.new("RGBA", (120, 72), (0, 0, 0, 0))
            m = k * 3
            parts = [(0, 0, m, m, 0, 0), (n * 3 - m, 0, n * 3, m, 120 - m, 0), (0, n * 3 - m, m, n * 3, 0, 72 - m),
                     (n * 3 - m, n * 3 - m, n * 3, n * 3, 120 - m, 72 - m)]
            edge_t = big.crop((m, 0, n * 3 - m, m)).resize((120 - 2 * m, m), Image.NEAREST)
            edge_b = big.crop((m, n * 3 - m, n * 3 - m, n * 3)).resize((120 - 2 * m, m), Image.NEAREST)
            edge_l = big.crop((0, m, m, n * 3 - m)).resize((m, 72 - 2 * m), Image.NEAREST)
            edge_r = big.crop((n * 3 - m, m, n * 3, n * 3 - m)).resize((m, 72 - 2 * m), Image.NEAREST)
            centre = big.crop((m, m, n * 3 - m, n * 3 - m)).resize((120 - 2 * m, 72 - 2 * m), Image.NEAREST)
            tile.paste(centre, (m, m))
            tile.paste(edge_t, (m, 0)); tile.paste(edge_b, (m, 72 - m)); tile.paste(edge_l, (0, m)); tile.paste(edge_r, (120 - m, m))
            for x0, y0, x1, y1, tx, ty in parts:
                tile.paste(big.crop((x0, y0, x1, y1)), (tx, ty))
            sheet.alpha_composite(tile, ((i % 6) * w + 6, (i // 6) * h + 6))
        sheet.save(sys.argv[1])
        print("sheet", sys.argv[1])


if __name__ == "__main__":
    main()
