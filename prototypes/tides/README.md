# Tides spike: a sea level that moves with Luna

**Question** (worldgen-plan W5, `docs/design/rivers-and-tides.md`, "Tides"): can the sea's level be a pure function
of place and time, 1 m from low to high at springs, followed by the shader, the physics and the live water,
with nothing stored per column for the still sea (ADR 0020), no remeshing for the tide, no flicker at the
waterline, the water ledger exact, and a cost that does not show in the budget (ADR 0004)?

**Answer: yes.** The spike was small enough to be the engine change itself (ADR 0064); this folder keeps the
question, the measured answers and the filmstrip script. What was measured:

| Question (design doc) | Answer |
|---|---|
| 1. Cost per vertex and per physics query | `Tide::at` 4.4 ns (two dot products, four multiplications). The moment's directions (`Sky::tide`, inside `Solid::at`) 148 ns, once per `World::ground()`. A sea column's `generated()` 249 ns at the mean, 340 ns with the tide (the column's direction is the new work). `cargo test --release -p engine tide_cost -- --ignored --nocapture`, on the 4 km Terra, quiet PC. |
| 2. Flicker at the waterline | The mean sea lies 1/8 under a layer top; a moving one does not. The shader keeps the level 1/64 layer off any top (it steps over it, 3 cm in an instant), so a surface never coincides with a block's top. |
| 3. A harbour through a 2-block gap | `liquid::tests::a_dug_harbour_follows_the_tide_in_and_out`: a harbour dug behind the shore fills to within 5 cm of high water and drains to low over two tides, `counted()` exact every step. Without the once-a-second `retide` it never moves (seen failing: stuck at 4.76 m against a sea of 5.5 to 6.5). |
| 4. Wet and dry sand | The tidal band is one ring of columns (ground's top the layer over the mean sea's), about 10 columns wide on the beach filmed. It reads well without a block of its own (`out/tides/high-low.png`); a wet-sand look is a taste call left open. |
| 5. River mouths | `water_at`'s rule (the sea wins where it stands over a river's level) does it: at high water a river's last columns are the sea's, at low water the river's again. No new code. |
| Volume | The bulge moves water, it makes none: `Tide::base` takes the bulges' mean over the actual ocean (its area's second moment, a 3 x 3 matrix from 4096 directions at load). On the 4 km Terra the ocean's shape moves the mean by up to 6.5 cm against a sea all around (base -0.40 to -0.31 m over a month, -0.333 round). |

**Range at the filmed beach** (31 degrees N, face 2, column 5221, 3399): 1.00 m at springs (day 12: -0.36 to
+0.64 m), 0.6 m at neaps. Two highs a lunar day (1920 s): 16 minutes apart.

## Filmstrips
`python -I prototypes/tides/film.py <technoape.exe> <out.png> <stand args> -- t1 t2 ...` renders one `run=stand`
frame per world time into a sheet with the time under each. The beach (from the `tide_cost` bench):

    at=0.071286,0.849390,0.522929 yaw=-9.1 pitch=-35 eye=5

- `out/tides/day.png`: a spring day, a frame a minute (21300 to 22200 s).
- `out/tides/high-low.png`: high (21840 s) beside low (22320 s).
- `out/tides/month.png`: the 16 days of a month, each day's daylight high (rows 1-2) and low (rows 3-4).
