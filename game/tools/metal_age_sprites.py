"""Sprites of the bronze and iron ages (ADR 0070), added to make_item_sprites.py's sprites by
add(globals()) just before it writes the JSON. Every tool, head and work piece reuses the shared
tool shapes with the metal's palette; a sprite another part already drew is left alone.
"""


def add(G):
    sp, SP, H, SH = G["sp"], G["SP"], G["H"], G["SH"]

    def once(item, *layers):
        if item not in SP:
            sp(item, *layers)

    heads = {"knife": "knife_blade", "axe": "axe_head", "shovel": "shovel_head", "hoe": "hoe_head",
             "pickaxe": "pickaxe_head", "hammer": "hammer_head", "saw": "saw_blade"}
    for m in ("bronze", "iron"):
        for t, shape in heads.items():
            handle = SH if t in ("knife", "saw") else H
            once("%s_%s" % (m, t), handle, (shape, m))
            once("%s_%s" % (m, "saw_blade" if t == "saw" else t + "_head"), (shape, m))
        once(m + "_work_piece", ("work_piece", m))
    once("crucible_bronze", ("crucible", "terracotta", {"i": "#f0c060"}))
    once("bronze_anvil", ("anvil", "bronze"))
