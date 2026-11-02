"""One-off data patch (ADR 0121): woven rugs and a straw mat, flat 1 x 1 m floor furniture.
Idempotent. Run from game/: python -I tools/content_rugs.py"""
import json
import re

ITEMS = "data/items.json"
RECIPES = "data/recipes.json"


def lines_of(path):
    return open(path, encoding="utf-8").read().split("\n")


def insert_after(lines, anchor_id, new_lines):
    have = {m.group(1) for l in lines for m in [re.match(r'\s*\{"id": "([a-z0-9_]+)"', l)] if m}
    new_lines = [l for l in new_lines if json.loads(l.rstrip(","))["id"] not in have]
    for i, l in enumerate(lines):
        if l.lstrip().startswith('{"id": "%s"' % anchor_id):
            lines[i + 1:i + 1] = new_lines
            return len(new_lines)
    raise SystemExit("anchor %s not found" % anchor_id)


def j(d):
    return "    " + json.dumps(d, ensure_ascii=False) + ","


RUGS = {"red_rug": ("Red Rug", ["wool", "wool", "wild_strawberry"]),
        "blue_rug": ("Blue Rug", ["wool", "wool", "cornflower"])}
items, recipes = lines_of(ITEMS), lines_of(RECIPES)
new_items, new_recipes = [], []
for rid, (name, parts) in RUGS.items():
    new_items.append(j({"id": rid, "name": name, "tags": ["furniture", "decoration"], "stack": 16, "furniture": rid, "fuel_ticks": 100,
                        "_note": "a woven wool rug dyed with %s: lies flat on the floor" % parts[2].replace("_", " ")}))
    new_recipes.append(j({"id": rid, "type": "shapeless", "ingredients": parts, "output": {"id": rid, "count": 1}}))
new_items.append(j({"id": "straw_mat", "name": "Straw Mat", "tags": ["furniture", "decoration"], "stack": 16, "furniture": "straw_mat",
                    "fuel_ticks": 100, "_note": "plaited dry grass: the stone age rug"}))
new_recipes.append(j({"id": "straw_mat", "type": "shaped", "pattern": ["GGG"], "key": {"G": "dry_grass"}, "output": {"id": "straw_mat", "count": 1}}))
n = insert_after(items, "flower_pot", new_items)
k = insert_after(recipes, "table", new_recipes)
open(ITEMS, "w", encoding="utf-8", newline="\n").write("\n".join(items))
open(RECIPES, "w", encoding="utf-8", newline="\n").write("\n".join(recipes))
for p in (ITEMS, RECIPES):
    json.load(open(p, encoding="utf-8"))
print("items +%d, recipes +%d" % (n, k))
