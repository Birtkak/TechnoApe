"""One-off data patch (ADR 0131, second pass): uses for the LV machines' outputs and the Workshop chapter
of the progression book. Idempotent. Run from game/: python -I tools/content_lv2.py
Power is written in watts (ADR 0123)."""
import json

ITEMS = "data/lv_items.json"
RECIPES = "data/lv_recipes.json"
QUESTS = "data/quests.json"


def load_lines(path):
    return open(path, encoding="utf-8").read().split("\n")


def add_rows(path, rows):
    lines = load_lines(path)
    have = {json.loads(l.strip().rstrip(","))["id"] for l in lines if l.lstrip().startswith('{"id"')}
    end = max(i for i, l in enumerate(lines) if l.lstrip().startswith('{"id"'))
    new = [r for r in rows if r["id"] not in have]
    if not new:
        return 0
    if not lines[end].rstrip().endswith(","):
        lines[end] = lines[end].rstrip() + ","
    lines[end + 1:end + 1] = ["    " + json.dumps(r, ensure_ascii=False) + "," for r in new]
    lines[end + len(new)] = lines[end + len(new)].rstrip(",")
    open(path, "w", encoding="utf-8", newline="\n").write("\n".join(lines))
    json.load(open(path, encoding="utf-8"))
    return len(new)


def set_fields(path, fields):
    lines = load_lines(path)
    for i, l in enumerate(lines):
        if not l.lstrip().startswith('{"id"'):
            continue
        comma = l.rstrip().endswith(",")
        d = json.loads(l.strip().rstrip(","))
        if d["id"] in fields:
            d.update(fields[d["id"]])
            lines[i] = "    " + json.dumps(d, ensure_ascii=False) + ("," if comma else "")
    open(path, "w", encoding="utf-8", newline="\n").write("\n".join(lines))


# what is already there gets its uses
set_fields(ITEMS, {
    "canned_meat": {"empties_to": "tin_can"}, "canned_fish": {"empties_to": "tin_can"}, "canned_vegetables": {"empties_to": "tin_can"},
    "sticky_resin": {"tags": ["resin", "binding", "tar"], "_note": "pressed out of a log in the extractor: binds a tool's haft like birch tar, makes raw rubber like tar, lights torches"},
})
n = add_rows(ITEMS, [
    {"id": "rubber_boots", "name": "Rubber Boots", "tags": ["clothing"], "stack": 1, "clothing": {"slot": "feet", "warmth_c": 2.0, "armor": 1},
     "_note": "dry, warm feet: four rubber"},
    {"id": "diving_mask", "name": "Diving Mask", "tags": ["clothing"], "stack": 1, "clothing": {"slot": "head", "warmth_c": 0.0, "armor": 0, "air_x": 3.0},
     "_note": "two lenses in a rubber seal: breath lasts three times as long under water"},
    {"id": "spyglass", "name": "Spyglass", "tags": ["optic"], "stack": 1, "zoom": 8.0,
     "_note": "two lenses in a copper pipe: held, the zoom key looks eight times closer"},
    {"id": "hand_drill", "name": "Hand Drill", "tags": ["tool", "pickaxe"], "stack": 1, "tier": 4, "durability": 1800, "speed": 1.8,
     "_note": "a steel pick head on a geared crank: almost twice a steel pick's speed with no steam"},
    {"id": "heavy_copper_cable", "name": "Heavy Copper Cable", "tags": ["cable", "station"], "stack": 64, "work": "cable",
     "cable": {"ohms_per_block": 0.00053, "amps": 75},
     "_note": "16 mm^2 of copper in rubber: under half the loss of the hide-sleeved cable and nearly twice its amps (ADR 0123)"},
])
k = add_rows(RECIPES, [
    {"id": "resin_torch", "type": "shapeless", "ingredients": ["sticky_resin", "#stick"], "output": {"id": "torch", "count": 4}},
    {"id": "rubber_boots", "type": "shaped", "pattern": ["R R", "R R"], "key": {"R": "rubber"}, "output": {"id": "rubber_boots", "count": 1}},
    {"id": "diving_mask", "type": "shaped", "pattern": ["RRR", "LRL"], "key": {"R": "rubber", "L": "glass_lens"}, "output": {"id": "diving_mask", "count": 1}},
    {"id": "spyglass", "type": "shaped", "pattern": ["L", "P", "L"], "key": {"L": "glass_lens", "P": "copper_pipe"}, "output": {"id": "spyglass", "count": 1},
     "station": "crafting_table"},
    {"id": "hand_drill", "type": "shaped", "pattern": [" H ", "GRG", " R "], "key": {"H": "steel_pickaxe_head", "G": "iron_gear", "R": "steel_rod"},
     "output": {"id": "hand_drill", "count": 1}, "station": "crafting_table", "_note": "ADR 0099: steel-age tools at the crafting table"},
    {"id": "heavy_copper_cable", "type": "shaped", "pattern": ["CCC", "RRR"], "key": {"C": "copper_cable", "R": "rubber"},
     "output": {"id": "heavy_copper_cable", "count": 1}},
    {"id": "belt_rubber", "type": "shaped", "pattern": ["RRR", "GSG", "PPP"], "key": {"R": "rubber", "G": "iron_gear", "S": "iron_rod", "P": "iron_plate"},
     "output": {"id": "belt", "count": 6}, "station": "crafting_table", "_note": "a rubber belt instead of leather: half as many again"},
])

# the Workshop chapter: the LV machines' side roads, the road to canned food in the middle
def q(id, title, icon, pos, deps, desc, items_, optional=True, final=False):
    out = {"id": id, "title": title, "icon": icon, "pos": pos, "deps": deps, "description": desc,
           "tasks": [{"type": "item", "item": i} for i in items_]}
    if optional:
        out["optional"] = True
    if final:
        out["final"] = True
    return out


book = json.load(open(QUESTS, encoding="utf-8"))
if not any(c["id"] == "lv_workshop" for c in book["chapters"]):
    book["chapters"].append({
        "id": "lv_workshop", "title": "Workshop", "icon": "extractor",
        "description": "More machines on the cables, each opening a road: rubber from a forest, metal parts without the anvil, food that keeps, ore refined to ingots (ADR 0131).",
        "quests": [
            q("extruder", "Extruder", "extruder", [0, 2], ["first_machine"], "Steel plates round a die and a piston: ingots pushed into pipes, gears, rods and tin cans, no anvil.", ["extruder"], optional=False),
            q("canning_machine", "Canning Machine", "canning_machine", [1, 2], ["extruder"], "Four tin ingots round the sealing head. A can of cooked food never spoils, and the empty can comes back when you eat.", ["canning_machine"], optional=False),
            q("canned_food", "Food That Keeps", "canned_meat", [2, 2], ["canning_machine"], "Can a meal: two cooked meat or fish, or three vegetables, in a tin can. Carry it into the deep caves.", ["#canned"], optional=False, final=True),
            q("extractor", "Extractor", "extractor", [0, 0], ["first_machine"], "Logs to sticky resin, resin to three rubber: rubber from a forest instead of tar and sulfur. Resin binds hafts and lights torches too.", ["extractor", "sticky_resin"]),
            q("rubber_boots", "Rubber Boots", "rubber_boots", [1, 0], ["extractor"], "Four rubber: warm, dry feet.", ["rubber_boots"]),
            q("heavy_cable", "Heavy Cable", "heavy_copper_cable", [1, 1], ["extractor"], "Three cables in a rubber sleeve: under half the loss over a long run and nearly twice the amps.", ["heavy_copper_cable"]),
            q("cutter", "Cutter", "cutter", [0, 4], ["first_machine"], "A steel saw blade on a motor: six planks a log, three lenses from a glass block, gears from plates.", ["cutter"]),
            q("spyglass", "Spyglass", "spyglass", [1, 4], ["cutter"], "Two lenses in a copper pipe. Hold it and press the zoom key to look eight times closer.", ["spyglass"]),
            q("diving_mask", "Diving Mask", "diving_mask", [1, 3], ["cutter"], "Two lenses in a rubber seal: your breath lasts three times as long under water.", ["diving_mask"]),
            q("hand_drill", "Hand Drill", "hand_drill", [2, 3], ["extruder"], "A steel pick head on a geared crank with steel rods: nearly twice a steel pick's speed, no steam.", ["hand_drill"]),
            q("electric_compressor", "Electric Compressor", "electric_compressor", [3, 0], ["coke_oven"], "Presses coke into carbon electrodes (coal works, four to one) and plant fibre into cordage.", ["electric_compressor", "carbon_electrode"]),
            q("electrolyzer", "Electrolyzer", "electrolyzer", [3, 1], ["electric_compressor", "macerator"], "Two crushed ore and an electrode make two ingots: macerated ore to metal with no crucible or bloomery.", ["electrolyzer"]),
        ]})
    open(QUESTS, "w", encoding="utf-8", newline="\n").write(json.dumps(book, indent=2, ensure_ascii=False) + "\n")
print("items +%d, recipes +%d" % (n, k))
