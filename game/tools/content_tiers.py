"""One-off data patch (ADR 0121): the steel tool set, bronze/iron/steel armour and metal spears.

Idempotent: skips ids that already exist. Run from game/: python tools/content_tiers.py
"""
import json
import re

ITEMS = "data/items.json"
RECIPES = "data/recipes.json"


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


items = lines_of(ITEMS)
recipes = lines_of(RECIPES)
by_id = {}
for l in items:
    if l.lstrip().startswith('{"id"'):
        d = json.loads(l.strip().rstrip(","))
        by_id[d["id"]] = d

# steel tools: iron's definitions at tier 4 with 1.6x the uses; heads smithed like iron's
new_items, new_recipes = [], []
heads = {"knife": "knife_head", "axe": "axe_head", "shovel": "shovel_head", "hoe": "hoe_head", "hammer": "hammer_head", "saw": "saw_blade"}
for tool, head in heads.items():
    new_items.append(j({"id": "steel_" + head, "name": "Steel " + head.replace("_", " ").title(), "tags": ["tool_head", "steel_head"], "stack": 64, "_note": "smithed only (ADR 0121)"}))
for tool, head in heads.items():
    d = dict(by_id["iron_" + tool])
    d["id"] = "steel_" + tool
    d["name"] = "Steel " + tool.title()
    d["tier"] = 4
    d["durability"] = int(d["durability"] * 1.6)
    d.pop("_note", None)
    new_items.append(j(d))
smith = {}
for l in recipes:
    m = re.match(r'\s*\{"id": "smith_iron_([a-z_]+)"', l)
    if m:
        smith[m.group(1)] = json.loads(l.strip().rstrip(","))
for tool, head in heads.items():
    r = dict(smith[head])
    r["id"] = "smith_steel_" + head
    r["input"] = "steel_ingot"
    r["output"] = {"id": "steel_" + head, "count": 1}
    new_recipes.append(j(r))
    new_recipes.append(j({"id": "steel_" + tool, "type": "shaped", "pattern": [" H ", " S ", " S "], "key": {"H": "steel_" + head, "S": "#stick"},
                          "output": {"id": "steel_" + tool, "count": 1}, "station": "crafting_table", "_note": "ADR 0099 / 0121: steel tools at the crafting table"}))

# armour: helmet, chestplate, boots for bronze, iron, steel (copper's patterns)
armour = {"bronze": (2, 6, 2), "iron": (3, 7, 3), "steel": (4, 8, 3)}
slots = (("helmet", "head", ["MMM", "M M"]), ("chestplate", "body", ["M M", "MMM", "MMM"]), ("boots", "feet", ["M M", "M M"]))
for m, pts in armour.items():
    for (piece, slot, pattern), a in zip(slots, pts):
        new_items.append(j({"id": "%s_%s" % (m, piece), "name": "%s %s" % (m.title(), piece.title()), "tags": ["clothing", "armor"], "stack": 1,
                            "clothing": {"slot": slot, "warmth_c": 0.0, "armor": a}}))
        new_recipes.append(j({"id": "%s_%s" % (m, piece), "type": "shaped", "pattern": pattern, "key": {"M": "#metal." + m},
                              "output": {"id": "%s_%s" % (m, piece), "count": 1}, "station": "crafting_table"}))

# spears: the flint spear's reach and throw, harder hitting per metal
spear = by_id["flint_spear"]
for m, tier, dur, hit, throw in (("bronze", 2, 160, 5, 9), ("iron", 3, 256, 6, 11), ("steel", 4, 400, 7, 13)):
    d = {"id": m + "_spear", "name": m.title() + " Spear", "tags": spear["tags"], "stack": 1, "tier": tier, "durability": dur,
         "attack_damage": hit, "throw": {"damage": throw, "speed_m": spear["throw"]["speed_m"]}}
    new_items.append(j(d))
    new_recipes.append(j({"id": m + "_spear", "type": "shaped", "pattern": ["  M", " S ", "S  "], "key": {"M": "#metal." + m, "S": "#stick"},
                          "output": {"id": m + "_spear", "count": 1}, "station": "crafting_table"}))

n = insert_after(items, "steel_pickaxe", new_items)
k = insert_after(recipes, "steel_pickaxe", new_recipes)
open(ITEMS, "w", encoding="utf-8", newline="\n").write("\n".join(items))
open(RECIPES, "w", encoding="utf-8", newline="\n").write("\n".join(recipes))
for p in (ITEMS, RECIPES):
    json.load(open(p, encoding="utf-8"))
print("items +%d, recipes +%d" % (n, k))
