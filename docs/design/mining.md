# Mining

- **Status:** design, 2026-11-13 (the developer's calls of that day; ADR 0087). Nothing here is built.
- **From:** vision (finite planets, a world that reacts), `worldgen.md` and ADR 0063 (rock by province, ore
  bodies as counted objects with a grade, found by eye), `fine-cells.md` and ADR 0079 (loose blocks fall,
  sand buries), ADR 0034 (a block is dug held, by its hardness), `progression.md` (drill, ore line,
  explosives), `air.md`, ADR 0074 (slots and piles).

## The idea
**The rock is something you read, and the mine is a place you build.** Minecraft's mining is a grid of
tunnels until ore shows; Vintage Story's is a prospecting tool that gives numbers. Ours is realism where it
makes play (the ground has a shape and a grain, water and waste are real) and no bookkeeping where it does
not (no weight).

## Five parts

### 1. Follow the body
An ore body is a shape with a rich middle and a lean rim (ADR 0063). The rock around it is visibly changed
(stained, veined with quartz), more so toward the core, so a face tells which way the body lies. Mining is
chasing that shape; strip-mining a grid finds less than reading a face. No ore map and no tool that prints
numbers; a hand drill that pulls a core of the layers below may come later.

### 2. Break rock with its grain
- **Cracks show on the face.** Layered rock (shale, limestone, sandstone) shows bedding lines, massive rock
  (granite, basalt) a few joints. Both are a rule over (rock, place), like the rock itself: computed, not
  stored.
- **A block dug on a crack loosens its neighbours along it:** a good hit drops 3 to 6 blocks, a bad one 1.
- **Undercut and drop:** dig out the soft layer under a hard one and the hard layer comes down under its own
  weight.
- **Blasting** (a later age, `progression.md`): drill a pattern of holes, load, light, run; the pattern
  decides what comes down.
- **A plain pick always works,** one block at a time and slow in hard rock (ADR 0034). Technique is a
  speed-up, never a wall. No waiting mechanics (fire-setting and wedges were turned down as dull).
- **A drop hurts** whoever is under it, as falling sand buries (ADR 0079).

### 3. The mine fights back
With systems the world already has or plans: **water** below the water table (a drainage tunnel, then pumps),
**heat** with depth (ADR 0063), **bad air** in dead ends (`air.md`; ventilation shafts), a **weak roof** in
loose rock (timber props; granite stands by itself).

### 4. Rubble, not weight
- **Broken rock is loose rubble that blocks the way:** real loose blocks (ADR 0079's falling kind), not
  piles to walk over. A drop buries the floor; the tunnel must be cleared.
- **Rock, rubble and ore stack to 16** (ADR 0074's stack size per item). A pack fills after one good drop.
- **Most of a mine is waste:** a lean rim is about 1 ore in 10 blocks. Moving rock is the job.
- **So the tools come in order:** basket, wheelbarrow, a hoist bucket up the shaft, chutes (gravity works for
  free), carts on rails, conveyors, the powered drill feeding a belt. This is where mining meets automation.
- **Spoil heaps** are where the rubble was put, and show from orbit (`fundamentals.md`).

### 5. Grade matters after the dig
Core ore smelts as it is. Rim ore pays only once it is crushed and washed (the ore line's two to three times
the metal, `progression.md`). The ledger (ADR 0063) already counts ingots per block.

## To find out before building (spikes, each with a number)
1. **Cracks on a face in the calm texture style** (ADR 0084): a look sheet for the developer.
2. **A mine full of loose rubble:** frame and tick cost of some thousands of resting and falling loose blocks
   (ADR 0079 measured 200 ns an awake block; resting ones must cost nothing).
3. **Natural rock that drops:** today only edited blocks are searched for support (ADR 0079 (c)); a drop along
   a crack or over an undercut needs a capped search of generated rock near a dig.
4. **Drop sizes and the 1 in 10:** tuned by playing, with the ore budget's totals unchanged.

## Order
After items and fine cells' collapse slice: (a) rubble as loose blocks and stacks of 16; (b) cracks and the
drop along them; (c) undercut; (d) the alteration halo around bodies; (e) water, air and roof as their systems
arrive; (f) hauling tools with the automation ages; (g) blasting with the chemical plant.
