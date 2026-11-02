"""One-off data patch (ADR 0121): cooking-pot meals with buffs. Idempotent. Run from game/."""
import json
import re



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

ITEMS = "data/items.json"
RECIPES = "data/recipes.json"

# id: (name, inputs [(item, count)], food, nutrition, buff, spoil_days)
MEALS = {
    "porridge": ("Porridge", [("wheat", 3), ("einkorn", 3), ("emmer", 3)], {"hunger": 6, "saturation": 7}, {"grain": 4},
                 {"kind": "warm", "amount": 6.0, "s": 300}, 2),
    "fish_soup": ("Fish Soup", [("raw_fish", 2)], {"hunger": 7, "saturation": 8}, {"meat": 3, "vegetable": 1},
                  {"kind": "haste", "amount": 0.15, "s": 120}, 2),
    "mushroom_soup": ("Mushroom Soup", [("brown_mushroom", 3)], {"hunger": 6, "saturation": 6}, {"vegetable": 3},
                      {"kind": "regen", "amount": 5.0, "s": 60}, 2),
    "berry_compote": ("Berry Compote", [("wild_strawberry", 4), ("blueberry", 4)], {"hunger": 4, "saturation": 4}, {"fruit": 4},
                      {"kind": "cool", "amount": 6.0, "s": 300}, 5),
}

items = lines_of(ITEMS)
recipes = lines_of(RECIPES)
new_items, new_recipes = [], []
for mid, (name, inputs, food, nut, buff, spoil) in MEALS.items():
    new_items.append(j({"id": "pot_of_" + mid, "name": "Pot of " + name, "tags": ["vessel"], "stack": 1,
                        "_note": "craft it with clay bowls: three servings, the pot comes back"}))
    new_items.append(j({"id": mid, "name": name, "tags": ["food", "meal"], "stack": 8, "food": food, "nutrition": nut, "buff": buff,
                        "spoil_days": spoil, "empties_to": "clay_bowl"}))
    for inp, n in inputs:
        new_recipes.append(j({"id": "%s_%s" % (mid, inp), "type": "melt", "station": "firepit", "input": inp, "count": n,
                              "vessel": "cooking_pot", "output": {"id": "pot_of_" + mid, "count": 1}, "ticks": 300}))
    new_recipes.append(j({"id": "serve_" + mid, "type": "shapeless", "ingredients": ["pot_of_" + mid, "clay_bowl", "clay_bowl", "clay_bowl"],
                          "remainder": {"pot_of_" + mid: "cooking_pot"}, "output": {"id": mid, "count": 3}}))

n = insert_after(items, "pot_of_stew", new_items)
k = insert_after(recipes, "serve_stew", new_recipes)
# the old stew warms too
for i, l in enumerate(items):
    if l.lstrip().startswith('{"id": "stew"') and '"buff"' not in l:
        d = json.loads(l.strip().rstrip(","))
        d["buff"] = {"kind": "warm", "amount": 4.0, "s": 240}
        d["tags"] = sorted(set(d["tags"]) | {"meal"})
        items[i] = j(d)
open(ITEMS, "w", encoding="utf-8", newline="\n").write("\n".join(items))
open(RECIPES, "w", encoding="utf-8", newline="\n").write("\n".join(recipes))
for p in (ITEMS, RECIPES):
    json.load(open(p, encoding="utf-8"))
print("items +%d, recipes +%d" % (n, k))
