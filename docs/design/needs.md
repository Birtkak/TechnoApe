# The ape's body: thirst, food, warmth, air, and what death costs

- **Status:** design proposal, 2026-11-11. `survival.md` and ADR 0040 are what is built (health, hunger,
  impact, a death screen); this is where they grow, in the order the engine can carry it. The developer's
  calls: survival is harsh, and every need starts as a chore and ends as a factory (ADR 0008). Lines
  marked *taste call* are open; the numbers are starting points for the first playtest and belong in
  `data/needs.toml`.

## One shape for every need
- Each need is a number from 0 to 20 on the server, in `Vitals`, drained by rates from data and counted
  per physics step the ape itself takes, as hunger is (so a time warp starves nobody).
- **A need bites in two steps: first it costs ability, then health.** Low, the ape is slower at something
  (below); empty, health drains. The player always gets a stretch of "this is getting bad" before "this
  is killing me".
- **The HUD shows a need only while it matters.** Health and hunger stay as they are; thirst joins them
  as a third bar. Breath appears while the ape cannot breathe, and warmth is a small sign that shows only
  outside the comfortable band. A calm ape in a meadow sees three bars.
- Every way to lose health is a cause in `Vitals::hurt`, so the death screen and the chat can always say
  what killed: an impact, thirst, hunger, cold, heat, drowning, vacuum, later a creature, poison,
  radiation.

## The needs

| Need | Drains | Low (under 6) | Empty | Filled by |
|---|---|---|---|---|
| **Hunger** (built) | a game day standing, half a day walking, a little per block dug | no sprint | health falls to 1, and below once food exists | food: its row says how many points |
| **Thirst** | two thirds of a day standing, a third walking; half again faster in heat | slower digging | health falls a point per 10 s | drinking |
| **Breath** | 30 s without air | – | health falls a point per second | air: full again in 2 s |
| **Warmth** | not a bar: a felt temperature against a comfortable band | cold: hunger drains up to twice as fast; heat: thirst does | far outside the band: health falls | fire, shelter, clothes, shade, water |

With these, an ape that walks half the time drinks a little over twice a day and eats three times in two
days: `open-questions.md`'s "thirst twice, food once", with hunger's built rates kept.

### Thirst
- **Drinking** is aiming at fresh water within reach and holding the use button: full in two seconds. It
  takes water from the world (a mouthful is far under a block; the ledger counts it as used and the ape
  gives it back to the air over time, with the water cycle).
- **Sea water does not help**: the sea is salt. It needs the water to know it: the generated sea is salt,
  water on land from the bake (rivers, lakes, rain) is fresh, and mixed water is salt when more than a
  small share of it came from the sea. One bit per water column is enough to start.
- A vessel (`items.md`) carries water away from the pond; that is the clay pot's whole point in the first
  hour. Later: dirty water (pollution) fills thirst and costs health until it is treated.
- Chore to factory: the pond, the pot, a hand pump and a tank, pipes and treatment.

### Food
- Food items have `feeds` (points) and a day they spoil (`items.md`). Raw plants and meat feed little;
  the same thing cooked feeds about twice as much, so fire pays at once.
- Spoiled food is not gone: it is compost (elements are conserved), which the farm wants.
- **Four food groups** (the developer's call, ADR 0043): **protein** (meat, fish, eggs, nuts, beans),
  **vegetables and fruit** as one, **dairy**, and **carbohydrates** (grain, roots, bread). A food's row
  names its group; a cooked dish of several foods counts for each.
- How they act is a *proposal*, to tune in play:
  - The body remembers when it last ate of each group. A group is **lit** for two game days after, shown
    as four small marks beside the hunger bar. No new bars.
  - **Variety rewards; it does not punish.** One group lit is the plain ape. Each further group lit makes
    hunger drain a tenth slower and health come back a fifth faster; all four add two points to the top
    of health. An ape living on berries is never hurt for it, only plain.
  - The reason for reward only: dairy needs herded animals, which the first hours do not have, and a
    penalty for a food that cannot yet exist is not a decision.
- So the groups are the food chain's ladder: forage (vegetables and fruit), the hunt or the snare
  (protein), the farm, mill and oven (carbohydrates), the herd (dairy). The moon base's life support loop
  must grow all four, or its apes live plain.
- Chore to factory: foraging, the cooking fire, the farm with mill and oven, the herd, the cold store, a
  kitchen line.

### Warmth
- The **felt temperature** is `air.md`'s local temperature: the air where the ape stands, plus nearby
  fires, lava and machines, or the room's own state inside one. Being wet (in water, in rain) and wind
  make cold worse; shade and water make heat better.
- The ape has a **comfortable band**, bare about 10° to 30°. Clothes widen it: hides for the cold side,
  later a suit that carries its own band anywhere. Outside the band the ape pays in hunger or thirst
  first, and in health only when it is far outside (below about -5° or above 45° bare) or stays out long.
- **The night must be a reason to build, not a wall.** The developer's call (ADR 0043): a bare ape on a
  temperate plain survives one night in the open, arriving at dawn hungry and hurt; a fire or a cave
  makes the night cost nothing; the poles and high mountains kill a bare ape in minutes at any hour.
  The air spike's question 6 (how strong the day's swing is) is tuned against this sentence.
- **It can ship before the air grid.** Stage one is a closed formula from what the sky already knows: a
  base by latitude, the day's swing by the sun's height, colder with altitude and warmer with depth. A
  fire warms the cells within a few blocks, and "sheltered" is a roof within a few blocks overhead (one
  column lookup). The air grid and rooms replace the formula later without changing what the player
  learned.
- Chore to factory: the fire, Home (the first megaproject is this need's measurement), a heated base, the
  suit and the sealed habitat.

### Breath
- The ape cannot breathe with its head under water, above where the air ends (1,500 m, where the sky
  already lets it end), on a body without air, and later in a room whose air is bad.
- So survival cannot yet live on Luna, and that is the point: the pressure suit (EV) is the air need in a
  bottle, the sealed habitat (IV) is the same need as a room. Until suits exist, a survival ape that
  flies up dies of it; creative has no needs at all.
- Thin air on high mountains is not modelled (a default: one more rule for little play).

## Each new place brings a need back

| Place | What it does to the ape | First answer |
|---|---|---|
| the night side | cold, dark | fire, a cave |
| the open sea, the coast | water everywhere and none to drink | a carried pot; later boiling it |
| deserts and the dry belt | heat, thirst half again faster, far from water | shade by day, travel by night |
| poles, high ranges | deadly cold at any hour | hides, then a heated camp |
| deep underground | warm, then hot; later bad air | the cave is the first shelter and the deep mine the first place that needs a fan |
| under water | no breath | 30 seconds; later tanks |
| Luna and orbit | no air, heat and cold with no air between them, radiation | the suit, then living under rock |

## What death costs (decided, ADR 0043)
- The ape's things become **a pile where it died** (`items.md`: nothing despawns), and a waypoint named
  for the death is dropped there by the game, in the death's own colour (ADR 0030 has the waypoints). It
  is removed when the pile is empty.
- The ape wakes at **Home**: the last sleeping place it claimed (a bed of leaves beside a fire is enough),
  else Terra's spawn. It wakes with full health and needs: the cost is the walk back, perhaps at night,
  with empty hands. No death spiral.
- Anyone may take from the pile. The game is played with friends on a home server; locks come with roles
  if they ever do.
- A sleeping place does not skip the night (a shared world has one clock). It is only where the ape wakes.
- **Dying on Luna** wakes the ape at its Home, wherever that is. Without a habitat that is Terra, and the
  things stay on Luna until the next flight.
- Creative flight stays unhurt (ADR 0040's default).

## What the engine needs, in the order to build
1. **Thirst and drinking:** the third bar, the salt bit on water, the use button on water. It needs
   nothing new but a fresh-water source near the spawn, which the bake's rivers and lakes give
   (`worldgen-plan.md` W3); until then, rain-fed ponds by hand with `/place`.
2. **Breath:** under water and above the air. Small, and it gives water its danger.
3. **Death's cost:** piles (`items.md` step 2), the death waypoint, the sleeping place.
4. **Food as items:** `feeds`, the group and the spoiling day (`items.md` step 6), plants to forage (the
   biomes), the cooking fire (the first process of `crafting.md`), the four marks. Hunger's floor of 1
   health goes. Protein waits for animals, dairy for herding (ADR 0027's creatures).
5. **Warmth, stage one:** the formula, the fire's reach, the roof check, the sign on the HUD, hides.
6. **Warmth, stage two:** the air grid's temperature and rooms.

## Tests
- Each need: its rates in `Vitals::step` as unit tests seen failing first, as hunger's are.
- **The balance is a test, not a feeling.** Scripted bots on a fixed seed play fixed lives, and the gate
  holds the day each dies or its state at dawn:
  - an ape that never acts dies of thirst late on the first day (thirst is empty at 800 s, health at
    1,000 s), not sooner;
  - an ape that only drinks lives through the first night hurt, and starves early on the second day;
  - an ape that drinks, forages and lights a fire reaches the second dawn with full health;
  - an ape fed from all four groups for two days has 22 health at its top and outlasts the plain ape's
    hunger by the stated share.
  A change to any rate that moves these is a change someone meant.
- The soak's invariant: no need outside 0..20, a dead ape takes no steps, and water drunk is in the
  water's ledger.
- F3's Player group and a play run's `vitals` JSON carry every need and the felt temperature.
