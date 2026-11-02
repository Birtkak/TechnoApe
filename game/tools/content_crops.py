"""One-off data patch (ADR 0121): potatoes, onions and beans as crops, a baked potato.
Idempotent. Run from game/: python -I tools/content_crops.py"""
import json
import re

ITEMS = "data/items.json"
RECIPES = "data/recipes.json"
CROPS = "data/crops.json"
MINING = "data/mining.json"


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


# id: (name, crop, food, wild drop chance from tall grass); each is its own seed like the carrot
CROP_ITEMS = {
    "potato": ("Potato", "potatoes", {"hunger": 1, "saturation": 0.6}, 0.015),
    "onion": ("Onion", "onions", {"hunger": 1, "saturation": 0.6}, 0.015),
    "beans": ("Beans", "bean_plants", {"hunger": 2, "saturation": 1.2}, 0.02),
}
items, recipes = lines_of(ITEMS), lines_of(RECIPES)
new_items = []
for iid, (name, crop, food, _) in CROP_ITEMS.items():
    new_items.append(j({"id": iid, "name": name, "tags": ["food", "crop"], "stack": 64, "plants": crop, "food": food,
                        "nutrition": {"vegetable": 2}, "spoil_days": 20, "_note": "wild ones turn up in tall grass; plant on farmland (ADR 0121)"}))
new_items.append(j({"id": "baked_potato", "name": "Baked Potato", "tags": ["food"], "stack": 64, "food": {"hunger": 5, "saturation": 6},
                    "nutrition": {"vegetable": 3}, "spoil_days": 4}))
new_recipes = [
    j({"id": "bake_potato_firepit", "type": "smelt", "input": "potato", "output": {"id": "baked_potato", "count": 1}, "ticks": 300, "station": "firepit"}),
    j({"id": "bake_potato_furnace", "type": "smelt", "input": "potato", "output": {"id": "baked_potato", "count": 1}, "ticks": 200, "station": "furnace"}),
]
n = insert_after(items, "carrot", new_items)
k = insert_after(recipes, "cook_meat_firepit", new_recipes)
open(ITEMS, "w", encoding="utf-8", newline="\n").write("\n".join(items))
open(RECIPES, "w", encoding="utf-8", newline="\n").write("\n".join(recipes))

crops_text = open(CROPS, encoding="utf-8").read()
carrot_looks = json.loads(crops_text)["crops"]["carrots"]["looks"]
for iid, (_, crop, _, _) in CROP_ITEMS.items():
    if '"%s": {' % crop in crops_text:
        continue
    entry = {"seed": iid, "frost_c": -2.0, "ages": 8, "looks": carrot_looks,
             "drops_ripe": [{"id": iid, "count": 1, "binomial": [3, 0.5714286]}], "drops_young": [{"id": iid}],
             "_note": "shortcut: drawn with the carrot looks (ADR 0121), own looks need material ids 217..227"}
    body = ",\n".join("      %s: %s" % (json.dumps(key), json.dumps(v)) for key, v in entry.items())
    end = crops_text.rindex("    }\n  }\n}")
    crops_text = crops_text[:end] + "    },\n    %s: {\n%s\n" % (json.dumps(crop), body) + crops_text[end:]
open(CROPS, "w", encoding="utf-8", newline="\n").write(crops_text)

mining = open(MINING, encoding="utf-8").read()
for iid, (_, _, _, chance) in CROP_ITEMS.items():
    drop = '{"id": "%s", "chance": %s}' % (iid, chance)
    if drop not in mining:
        a = '    "tall_grass":   {"hardness": 0.0, "drops": ['
        assert a in mining
        mining = mining.replace(a, a + drop + ", ", 1)
open(MINING, "w", encoding="utf-8", newline="\n").write(mining)
for p in (ITEMS, RECIPES, CROPS, MINING):
    json.load(open(p, encoding="utf-8"))
print("items +%d, recipes +%d" % (n, k))
