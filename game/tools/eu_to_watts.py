"""Converts the electrical age's data from EU (ADR 0113/0117) to real units (ADR 0123). Re-runnable: a file
already in watts is left as it is, so whichever lane lands second after a merge just runs it again.

  python game/tools/eu_to_watts.py

- every *items.json: "machine": {"eu_t": n}  -> "watts": n * 25 (16 EU/t = 400 W); the battery's "eu" -> "battery": {"wh"};
  a cable gets "cable": {"ohms_per_block", "amps"}; eu_meter becomes multimeter.
- every *recipes.json: "info": "8 EU/t..." -> "200 W..."; the eu_meter recipe id/output -> multimeter.
- quests.json: "n EU/t" in prose -> "n*25 W", "EU Meter" -> "Multimeter", "EU" -> "watts".
"""
import json
import os
import re

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "..", "data")
W_PER_EU = 25.0


def eu_text(s):
    s = re.sub(r"(\d+(?:\.\d+)?) EU/t", lambda m: "%d W" % round(float(m.group(1)) * W_PER_EU), s)
    s = s.replace("EU Meter", "Multimeter").replace("eu_meter", "multimeter").replace(" EU", " watts")
    return s


def load(name):
    p = os.path.join(DATA, name)
    with open(p, encoding="utf8") as f:
        return p, f.read()


def save(p, text):
    with open(p, "w", encoding="utf8", newline="\n") as f:
        f.write(text)


def items(name="items.json"):
    p, text = load(name)
    d = json.loads(text)
    changed = 0
    for it in d["items"]:
        m = it.get("machine")
        if isinstance(m, dict) and "eu_t" in m:
            m["watts"] = round(float(m.pop("eu_t")) * W_PER_EU)
            changed += 1
        if it["id"] == "battery_box" and "battery" not in it:
            it["battery"] = {"wh": 2000}
            changed += 1
        if it["id"] == "copper_cable" and "cable" not in it:
            it["cable"] = {"ohms_per_block": 0.0014, "amps": 40}
            changed += 1
        if it["id"] == "electric_lamp" and "load" not in it:
            it["load"] = {"watts": 40}
            changed += 1
        if it["id"] == "pump" and "load" not in it:
            it["load"] = {"watts": 200}
            changed += 1
        if it["id"] == "eu_meter":
            it["id"] = "multimeter"
            it["name"] = "Multimeter"
            changed += 1
        if "_note" in it:
            n2 = eu_text(it["_note"])
            if n2 != it["_note"]:
                it["_note"] = n2
                changed += 1
    if changed:
        # keep the file's one-item-per-line layout: rewrite only the changed items in place
        out = text
        for it in d["items"]:
            pat = re.compile(r'^    \{"id": "%s",.*\},?$' % re.escape(it["id"] if it["id"] != "multimeter" else "eu_meter"), re.M)
            line = "    " + json.dumps(it, ensure_ascii=False)
            m = pat.search(out)
            if m:
                out = out[:m.start()] + line + ("," if m.group(0).endswith(",") else "") + out[m.end():]
        save(p, out)
    print(name + ":", changed, "changes")


def recipes(name="recipes.json"):
    p, text = load(name)
    out = eu_text(text)
    out = out.replace('"id": "eu_meter"', '"id": "multimeter"')
    if out != text:
        save(p, out)
    print(name + ":", "changed" if out != text else "already in watts")


def quests():
    p, text = load("quests.json")
    out = eu_text(text)
    if out != text:
        save(p, out)
    print("quests.json:", "changed" if out != text else "already in watts")


if __name__ == "__main__":
    for f in sorted(os.listdir(DATA)):
        if f.endswith("items.json"):
            items(f)
        elif f.endswith("recipes.json"):
            recipes(f)
    quests()
