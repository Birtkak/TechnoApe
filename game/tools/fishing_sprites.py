"""Sprites for fishing and traps (ADR 0163): the rod's fish species, the snare and the pit trap
cover, added to make_item_sprites.py by add(globals()). Own file so it merges cleanly."""


def add(G):
    art, outline, shape, sp, ramp, PALETTES = G["art"], G["outline"], G["shape"], G["sp"], G["ramp"], G["PALETTES"]
    PALETTES.update({
        "trout": ramp("#2a2414", "#5e5430", "#8e8250", "#b8aa74", "#e8dcb0"),
        "salmon_fish": ramp("#2a1a20", "#6a4a52", "#a87a80", "#d0a8a4", "#f4d8cc"),
        "pike": ramp("#14200e", "#2e4a1e", "#4e7032", "#7a9a4e", "#c8d898"),
        "carp": ramp("#3a2008", "#7a4a14", "#b07a28", "#d4a044", "#f0d080"),
        "cod": ramp("#2a2a20", "#5a5a44", "#8a8a6a", "#b4b494", "#e4e4cc"),
        "mackerel": ramp("#0a1e2a", "#1e4a5e", "#3a7a8a", "#7ab0b4", "#e0f0ec"),
    })
    for item, pal in (("brown_trout", "trout"), ("salmon", "salmon_fish"), ("pike", "pike"), ("carp", "carp"),
                      ("cod", "cod"), ("mackerel", "mackerel")):
        sp(item, ("fish", pal))
    shape("snare", outline(art([
        "................", "..........ww....", "..........w.w...", "..........w..w..",
        "..........w.....", "..........w.....", "....mmmm..w.....", "...m....m.w.....",
        "..m......mw.....", "..m......m......", "..m......m......", "...m....m.......",
        "....mmmm........", "................", "................", "................"])),
        {"w": "#8a6440"})
    sp("snare", ("snare", "twine"))
    shape("pit_cover", outline(art([
        "................", "................", "................", "...m...m...m....",
        "..mmmmmmmmmmmm..", "...m.d.m.d.m....", "..mmmmmmmmmmmm..", "...md..m..dm....",
        "..mmmmmmmmmmmm..", "...m...m...m....", "................", "................",
        "................", "................", "................", "................"])),
        {"d": "#c8aa5a"})
    sp("pit_cover", ("pit_cover", "wood"))
