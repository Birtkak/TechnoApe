# Open questions: what is decided, what is still the developer's to call

- **Status:** living list, 2026-11-10. It gathers the open taste calls from `vision.md`,
  `progression.md` and `worldgen.md` in one place, with the options and a recommendation for each.
  The four calls made on 2026-11-10 are recorded in ADR 0027.

## Decided 2026-11-10 (ADR 0027)

| Question | The call | What follows |
|---|---|---|
| Luna's size | **about 1 km in radius** (a quarter of Terra, the real proportion) | N = 3072 gives 1,061 m: 14 km² of surface, 40 billion blocks. It must orbit farther out or it fills the sky: *proposal* 30 km away, 4° across. The orbit's distance and so its look are still to tune by eye. |
| How the ape learns | **by doing and seeing**; nothing is locked | a journal that fills in from what happens in the world; no research tree |
| What lives on Terra | **animals and hostile creatures** | hunting, herding and farming; and defence and weapons become part of the game |
| The 8 cube corners | **something made** | ancient structures or a mystery at each corner: a story thread, and lore to write |

## Opened by those calls

### What the hostile creatures are
The call makes danger a part of the game beside cold, night and hunger. Still open:
1. **Wild predators only, made bolder:** packs at night, things in caves and deep water. No new lore.
2. **Something tied to the corners' makers** (*recommended*): what guards or leaks from the made things at
   the corners. One story explains both calls; the threat grows as the ape digs deeper and reaches
   farther, so it scales with the tiers and has a reason to care about factories (noise, smoke, power).
3. **Raiders:** other apes or troops. Reads well with the AI troop, but needs believable AI early.

Also open: do they attack bases (walls, traps and turrets become content) or only apes; do they exist on
Luna; how much of an evening is fighting against building (*recommended:* a pressure you can design
away, as every need is, not a second game).

### What the made things at the corners are
- Who made them, and are they gone? What does each corner hold: one piece of a whole (eight parts of one
  machine), or eight different things?
- *Recommended shape:* the eight together are the reason the exotic age exists. The raw exotic of
  `progression.md` ("found only off the planet") and the knowledge to refine it are learned from them,
  one corner at a time, so the corners pace the whole game and the Ark has a story.
- They must be drawn from orbit (the summary pyramid of `world-data.md`) and must survive on a planet
  that can be mined out: are they diggable?
- Luna's 8 corners: the same makers, or empty?

### Luna at 1 km
- Its orbit's distance (30 km *proposed*), and with it how wide a total eclipse's band is on Terra.
- Its gravity (today "lunar" by data), its day, and whether the trip in the `orbit` benchmark keeps its
  length.
- `ore-budget.md` and `worldgen.md` now have room for maria, crater fields and real mining there.

## Still open, with a recommendation each

| Question | Options | Recommended |
|---|---|---|
| **How hands craft** | in the world (place things, strike, pour) / a crafting grid in the inventory / both by age | in the world through the stone age, a workbench surface after; no grid. It is the same rule as learning by seeing. |
| **How fast needs drain** | per day (drink twice, eat once) / faster / slower | per 30 minute day: thirst twice, food once, so a day's routine exists; tuned in the first playtest |
| **The AI troop** | other apes who gather and build beside you / only later / never | later: after the starter content, as a use of the bots' protocol |
| **The exotic resource's rule** | decays unless kept cold / unstable near power / needs constant flow | with the corners' story: decide them together |
| **How hard pollution and radiation punish** | permanent damage / slow natural recovery | recovery, slow alone and fast with a cleanup factory (progression's proposal) |
| **Tiers above IV, and what the next planet changes** | – | leave until the moon base exists |

## Opened 2026-11-11 by `items.md`, `needs.md` and `journal.md`

| Question | Options | Recommended |
|---|---|---|
| **What the food groups do** (the four groups are decided) | reward variety only / also punish a group long missed | reward only (`needs.md`): dairy cannot exist in the first hours |

Decided the same day (ADR 0047): the ape carries in the hotbar's 9 slots and rows earned by packs; death
drops a pile and a waypoint where the ape died, and it wakes at its sleeping place with full needs; the
first night in the open hurts and does not kill; each ape has its own journal; a dug block falls to the
ground and is walked over; a worn-out tool leaves nothing; food has four groups (protein, vegetables and
fruit, dairy, carbohydrates); a known journal entry is exact.

## Taste calls waiting inside the world generation plan
From `worldgen.md`; each stage of `worldgen-plan.md` stops for its own:
~~relief (how high, how deep)~~ decided 2026-11-11: ranges to 120 m, seas to 60 m, plains raised (median
land 20 m on seed 1), and the bake's file size matters less than its speed; how deep the ground goes; the
~~look of the default Terra~~ decided 2026-11-11: seed 5 of the sheet of 12 in
`prototypes/world-bake/images/` (the engine's default seed moves to it in W2); the wait when a
world is made (Terra bakes in about 18 s, the huge planet in about 105 s: is that fine?); the tilt and how strong seasons are; the tide's height; ~~the biome list~~ decided 2026-11-11, the design's 15 (ADR 0042), each biome's look still open; Luna's
ice; how much the player is told (map, compass, rock names).
