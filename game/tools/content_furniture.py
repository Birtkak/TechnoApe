"""One-off data patch (ADR 0121): plank furniture in every wood, a bench, a candle and a barrel.
Idempotent. Run from game/: python -I tools/content_furniture.py"""
import json
import re

ITEMS = "data/items.json"
RECIPES = "data/recipes.json"
MINING = "data/mining.json"
SOUNDS = "data/sounds.json"
WOODS = ["spruce", "acacia", "jungle", "birch"]
WOODED = ["door", "trapdoor", "fence", "fence_gate", "table", "bench", "shelf", "sign", "window"]   # FurnitureEntity.WOODED


def lines_of(path):
    return open(path, encoding="utf-8").read().split("\n")


def ids_in(lines):
    return {m.group(1) for l in lines for m in [re.match(r'\s*\{"id": "([a-z0-9_]+)"', l)] if m}


def insert_after(lines, anchor_id, new_lines):
    have = ids_in(lines)
    new_lines = [l for l in new_lines if json.loads(l.rstrip(","))["id"] not in have]
    for i, l in enumerate(lines):
        if l.lstrip().startswith('{"id": "%s"' % anchor_id):
            lines[i + 1:i + 1] = new_lines
            return len(new_lines)
    raise SystemExit("anchor %s not found" % anchor_id)


def j(d):
    return "    " + json.dumps(d, ensure_ascii=False) + ","


def row(lines, rid):
    for i, l in enumerate(lines):
        if l.lstrip().startswith('{"id": "%s"' % rid):
            return i, json.loads(l.strip().rstrip(","))
    return -1, None


items = lines_of(ITEMS)
recipes = lines_of(RECIPES)

new_items = [
    j({"id": "bench", "name": "Bench", "tags": ["furniture", "decoration"], "stack": 16, "furniture": "bench", "fuel_ticks": 300,
       "_note": "a two-seat plank bench: right click sits down"}),
    j({"id": "candle", "name": "Candle", "tags": ["furniture", "decoration", "light"], "stack": 16, "furniture": "candle",
       "light": {"color": "#ffc070"}, "_note": "a tallow candle: lights like a lantern, placed or held"}),
    j({"id": "barrel", "name": "Barrel", "tags": ["container", "storage"], "stack": 16, "material": "barrel", "fuel_ticks": 300,
       "_note": "18 slots: between the storage pot (9) and the chest (27)"}),
    j({"id": "sign", "name": "Sign", "tags": ["furniture", "decoration"], "stack": 16, "furniture": "sign", "fuel_ticks": 100,
       "_note": "click a wall to hang it, right click to write on it"}),
]
new_recipes = [
    j({"id": "bench", "type": "shaped", "pattern": ["PPP", "SSS"], "key": {"P": "oak_planks", "S": "#stick"},
       "output": {"id": "bench", "count": 1}, "station": "crafting_table"}),
    j({"id": "candle", "type": "shapeless", "ingredients": ["animal_fat", "cordage"], "output": {"id": "candle", "count": 2}}),
    j({"id": "barrel", "type": "shaped", "pattern": ["PMP", "P P", "PMP"], "key": {"P": "#planks", "M": "#metal.copper"},
       "output": {"id": "barrel", "count": 1}, "station": "crafting_table", "_note": "copper hoops (ADR 0121)"}),
    j({"id": "sign", "type": "shaped", "pattern": ["PPP", "PPP", " S "], "key": {"P": "oak_planks", "S": "#stick"},
       "output": {"id": "sign", "count": 3}, "station": "crafting_table"}),
]
# the oak recipes take oak planks now, so each wood's planks make their own (any layout matches, ADR 0046)
for k in WOODED:
    if k in ("bench", "sign", "window"):   # new, or not made of planks (the oak window is sticks and glass)
        continue
    i, r = row(recipes, k)
    if r["key"].get("P") == "#planks":
        r["key"]["P"] = "oak_planks"
        recipes[i] = j(r)
fresh = {json.loads(l.strip().rstrip(","))["id"]: json.loads(l.strip().rstrip(",")) for l in new_items}
fresh_r = {json.loads(l.strip().rstrip(","))["id"]: json.loads(l.strip().rstrip(",")) for l in new_recipes}
for w in WOODS:
    for k in WOODED:
        _, base = row(items, k)
        base = base or fresh[k]
        d = {key: v for key, v in base.items() if key != "_note"}
        d["id"] = "%s_%s" % (w, k)
        d["name"] = "%s %s" % (w.title(), base["name"].replace("Wooden ", ""))
        d["furniture"] = d["id"]
        new_items.append(j(d))
        _, r = row(recipes, k)
        if r is None:
            r = fresh_r[k]
        r = dict(r)
        r.pop("_note", None)
        r["id"] = "%s_%s" % (w, k)
        r["key"] = dict(r["key"], P="%s_planks" % w)
        if k == "window":   # a plank frame round two panes: the wood shows which window it is
            r["pattern"], r["key"] = ["PGP", "PGP"], {"P": "%s_planks" % w, "G": "glass"}
        r["output"] = {"id": "%s_%s" % (w, k), "count": r["output"]["count"]}
        new_recipes.append(j(r))

n = insert_after(items, "flower_pot", new_items)
k = insert_after(recipes, "table", new_recipes)
open(ITEMS, "w", encoding="utf-8", newline="\n").write("\n".join(items))
open(RECIPES, "w", encoding="utf-8", newline="\n").write("\n".join(recipes))

mining = open(MINING, encoding="utf-8").read()
if '"barrel"' not in mining:
    a = '    "chest":        {"hardness": 2.5, "tool": "axe", "drops": "chest"},\n'
    assert a in mining
    mining = mining.replace(a, a + '    "barrel":       {"hardness": 2.5, "tool": "axe", "drops": "barrel"},\n')
    open(MINING, "w", encoding="utf-8", newline="\n").write(mining)
sounds = open(SOUNDS, encoding="utf-8").read()
if '"barrel"' not in sounds:
    sounds = sounds.replace('"chest": "wood",', '"chest": "wood", "barrel": "wood",', 1)
    open(SOUNDS, "w", encoding="utf-8", newline="\n").write(sounds)
for p in (ITEMS, RECIPES, MINING, SOUNDS):
    json.load(open(p, encoding="utf-8"))
print("items +%d, recipes +%d" % (n, k))
