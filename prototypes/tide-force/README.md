# Tide-force spike: derived tides, and the tide as a head term on the water's links

**Question** (`docs/design/liquids.md`, "Forces" and steps 1-2; ADR 0177: tides derived from the bodies, no hand-set
amplitude, no scale factor):

- **A.** What tides do the rows of `data/bodies.toml` imply, by the engine's own radii, masses, pulls, distances
  and periods, for every block body (Terra, Luna, Mars, Callisto) and every raiser?
- **B.** Does the tide work as a force in the water (`liquid.rs`, ADR 0020): a per-column head offset eta added to
  the surface difference that speeds each link? Four tests, each seen failing without it; its cost on 100,000
  stored columns; how many stay awake through a tide, and which sleep rule keeps it cheap.

**Answer, short.** A: Terra's derived tide is **6.3 mm at springs** (today's constant gives 1 m), and the
**sun's is ten times Luna's** (0.59 mm Luna, 5.76 mm sun), because the engine's year is short and the sun's mass
that Kepler's law needs for it is huge. Callisto carries a fixed 1.86 m bulge under the giant. Luna's and
Callisto's orbits run 1.9 and 1.5 times faster than their parents' masses allow. B: **yes, but not through the
links.** On a 4 km planet a tide's slope is under one water unit a column (0.18 mm per column at 1 m springs), so
the link step's stop rule (`VSTOP`) swallows it: the term on links alone moves nothing (probe below). What works is
the same head (surface - eta) used in `settle` and in a once-a-second re-lay of sleeping water: all four tests
pass that way and fail without it; the cost is one eta a node (none measurable) and a re-lay of sleeping bodies.

## A. The derived tides

`cargo run --release` (the world of generation 7 on the 4 km Terra, `World::with_grid(6144, ..)`). The engine's
gravitational constant k is scaled so Terra pulls 19.62 m/s^2 (`interior::G`, ADR 0132): k = 2.24e-7, and with it
Terra's `weight_g` and `surface_g` are the same `k M / R^2`, so no correction is needed for Terra. The equilibrium
rise `h = (M_k / M) (R / d)^3 R` holds for the pull `k M / R^2`; the water moves by the felt pull, which phys takes
from `surface_g` (`World::gravity`, frames.rs), so h is used as it is. Only Luna's `weight_g` differs (19.62, set
for her rock): with it her tides would be 3.7 times smaller (column "with weight_g").

The sun has no row and no mass. Its mass here is the one Kepler's law needs for Terra's `YEAR` at her `sun`
distance: 1.86e22 kg, 11.7 million Terra masses, in the 435 km radius the eclipse rule gives it: **53,800 kg/m3**,
38 times the real Sun's density.

| body | radius m | mass / Terra's | surface_g | weight_g | sidereal period s | Kepler from the masses s | ratio |
|---|---|---|---|---|---|---|---|
| Terra | 4245 | 1 | 19.62 | 19.62 | 97,200 (YEAR) | 97,200 (sets the sun's mass) | 1 |
| Luna | 1150 | 0.0199 | 5.31 | 19.62 | 22,217 | 42,535 | **0.52** |
| Mars | 2300 | 0.126 | 8.40 | 8.40 | 194,400 | 179,126 | 1.09 |
| Jupiter | 46,700 | 300.7 | 48.75 | 48.75 | 777,600 | 1,086,715 | **0.72** |
| Callisto | 1061 | 0.0059 | 1.85 | 1.85 | 2,859 | 4,400 | **0.65** |

(A moon's pose turns `1/period + 1/YEAR`, `sky::circle`, so `MONTH` is Luna's month against the sun and her
sidereal period 22,217 s.)

Equilibrium tides, range = 1.5 h (high under the raiser to low 90 degrees off, at the equator, raiser in the
equator plane); "highs every" is half the raiser's day as the body's frame sees it:

| body | raiser | M_k / M | d / R | range | with weight_g | highs every | |
|---|---|---|---|---|---|---|---|
| Terra | sun | 1.17e7 | 23,500 | **5.76 mm** | | 900 s | sun's mass from Kepler |
| Terra | Luna | 0.0199 | 60 | **0.586 mm** | | 960 s | |
| Terra | Jupiter (closest) | 300.7 | 94,000 | 2.3e-9 m | | | |
| Luna | Terra | 50.3 | 221.5 | **7.98 mm** | 2.16 mm | fixed | locked: a fixed bulge |
| Luna | sun | 5.9e8 | 86,769 | 1.56 mm | 0.42 mm | 14,400 s (MONTH / 2) | what still varies |
| Mars | sun | 9.3e7 | 65,212 | 1.16 mm | | 97,200 s | Mars does not turn (no `day`): highs half a year apart |
| Callisto | Jupiter | 50,969 | 352 | **1.86 m** | | fixed | locked: a fixed bulge |
| Callisto | sun | 2.0e9 | 470,000 | 0.03 mm | | 1,435 s | what still varies |

Every other pair (planets at their closest) is under 1e-8 m.

- **Terra against today:** `TIDE_LUNA` 0.5 m (0.75 m range), the sun a third (0.25 m), 1 m at springs. Derived:
  0.59 mm and 5.76 mm, 6.3 mm at springs, 3 % of the way to a visible tide; the sun is 9.8 times Luna (the real
  Earth's 0.46). Why: the sun's term is `omega_year^2 R^2 / g`, fixed by the year alone, and the year is only 3.4
  Kepler months long (the real one 13.4); Luna's is small because the planet is 1500 times smaller than the
  Earth while Luna sits 60 radii out.
- **What the rows would need** (the rule: a row changes, not the physics): Luna's 0.75 m range needs her orbit at
  **5.5 Terra radii** (60 now; the fluid Roche limit for her density is 2.4), or her mass 1281 times (impossible
  for a body 3.7 times smaller). At 5.5 radii she hangs about 7 degrees wide (0.6 now) and the eclipse rule shrinks the sun to
  match: a taste call.
- **Locked moons:** the parent's bulge is fixed in the ground (Luna 8 mm, Callisto 1.86 m under the giant's point
  and -0.62 m at the ring 90 degrees off); the bake's sea level should carry it, the simulation never needs it as a
  moving term. Orbits are circles (no eccentricity, no libration), so what varies is the sun's alone: 1.6 mm on
  Luna every 4 h, 0.03 mm on Callisto. A real Callisto's eccentricity tide is absent by construction.

**Inconsistencies found (findings, not fixed):**
1. The sun has no mass; the one Kepler's law needs gives it 38 times the Sun's density.
2. Luna's month is set (`MONTH`, 16 days) and does not follow from Terra's mass: she circles 1.9 times faster
   than Kepler allows (Terra would need 3.7 times her mass). Callisto's period is scaled from Luna's
   (`frames.rs`), so it inherits the error (0.65).
3. `sky::period` rounds every year to a whole fraction of `PERIOD`: Mars's year is 8.5 % off Kepler, and the giant's
   is clamped to `PERIOD` itself (its Kepler year is 1.4 `PERIOD`s), 0.72.
4. A moon's pose turns `1/period + 1/YEAR` with the **home's** YEAR, also for Callisto, whose parent's year is
   eight times longer: its sun-relative day is a little off.
5. Mars has no `day`, so it does not turn: its sun stands still for half a year at a time.
6. Luna's `weight_g` (19.62) is not her pull (5.31); harmless for tides if water uses `surface_g`, as phys does.
7. `liquid.rs` flows by 9.81 (`CG`, `fall_cap`) while Terra pulls 19.62: water on Terra runs at half its gravity.

## B. The tide as a force

`src/sim.rs` is the smallest stand-alone copy of `liquid.rs`'s link step on one open gap a column, flat floors:
the same fixed point (U, VF, CG with g = 9.81), speed-up, friction, thin-sheet drag, VSTOP, QUIET sleep, `settle`
every 15 steps (calm and under 3 cm: laid flat, or to one sea neighbour's level), and an "open sea" column held at
mean + eta with the difference booked to `sea_extra` (liquid.rs's open ends). Left out: ceilings and pressure,
falls, carried speed, rivers, the wire, `forget_matching`. Added: eta per column (units << 8, from the raisers'
directions on a sphere of Terra's 4245 m radius), and:

- **the term:** a link speeds up by `(surface_a - eta_a) - (surface_b - eta_b)`;
- **tide settle:** a calm body is laid to its equilibrium (surface - eta uniform, units exact; beside the sea, each
  column to mean + eta there) instead of flat;
- **re-lay:** once a second, a sleeping body whose equilibrium moved a band or more since it was laid is laid
  again in place, without waking (closed: only the change's shape counts; beside the sea: all of it, booked to
  `sea_extra`). This replaces `retide`'s edge wake.

"Today" is term off, flat settle, edge wake. The tests run a 1 m springs tide (h = 2/3 m, a lunar day 1920 s) so
there is something to see; at the derived 6 mm every difference below is 160 times smaller and under the wire's
dead band. Band for the tests: 1/256 block (3.9 mm). `cargo test --release -- --nocapture`:

| test | today (without the term) | proposed | pass |
|---|---|---|---|
| 1. a 200-column harbour beside the sea, two tides: worst stored column off the sea's level | 49.6 mm (FAIL) | 4.4 mm | under DEAD (15.6 mm) |
| 2. a closed 200-column basin: end-to-end tilt off the potential's (which reaches 46.9 mm) | 46.9 mm, it stays flat (FAIL) | 7.8 mm; units drift 0 | yes, exact |
| 3. 200 x 5 band behind a mouth, a block set in at a quarter tide: its neighbours off the sea | 26.4 mm (FAIL) | 3.7 mm; ledger 0 | yes |
| 4. rest rule: a stretch laid at the formula, 60 s, tide frozen at its steepest | 1 m: still (the slope is inside VSTOP); 10 m: vmax 2850, 234 mm off (FAIL) | vmax 0, 0 mm off | yes |

The term on links alone, without the tide settle (`probe::links_alone`, `--ignored`): the closed basin keeps
the shape it was laid with for two whole tides, every node asleep, vmax 0. The tide's slope a column (0.96
units at 1 m springs, 0.006 at the derived size) speeds a link by under one velocity unit a step against
`VSTOP` = 4 and the friction's minimum 1; a link only moves for 7 units (1.7 mm) a column at g 9.81, a range of
about 7.3 m on this planet (3.6 m at 19.62; on Callisto, g 1.85 and radius 1061 m, 9.6 m). **So the head term is kept for moving
water, but the rest state and the sleep rule carry the tide.**

### Cost

`cargo run --release -- bench` (no fat LTO, no target-cpu=native; the PC shared with other sessions' builds, so
a few percent of noise):

**A step, 100,000 stored columns all awake** (500 x 200, kept awake, two raisers; three runs each):

| | ms a step | ns a node |
|---|---|---|
| without the term | 9.35 / 9.48 / 9.58 | 94 |
| with the term (eta worked out per node per step) | 10.05 / 10.15 / 10.18 | 101 |

The term costs **+0.65 ms a step per 100,000 awake nodes (+7 %)**, nearly all of it the eta itself; taken once a
second (step 2b below) it falls to the one subtraction a link. (liquid.rs's own step: 12 ms for 54,000 awake, the
comment in `step`; the copy is lighter.)

**Who stays awake through a tide** (high to high, 960 s), a 100,000-column basin 500 x 200 with the open sea along
its short edge (200 sea columns, always awake), laid at the formula at the start, 1 m springs:

| rule | mean awake | peak | re-laid columns a second | ms a step (mean) | worst column off the sea |
|---|---|---|---|---|---|
| today: no term, flat settle, edge wake | **67,800** | 100,033 | 0 | 6.33 | **388 mm** |
| term + tide settle, edge wake | 247 | 600 | 0 | 0.088 | 2.4 mm |
| term + tide settle + re-lay asleep, band 1/64 | 247 | 600 | 0 | 0.096 | 2.4 mm |
| the same, band 1/256 | 245 | 473 | 4,583 | 0.098 | 2.4 mm |
| the same, band 1/1024 | 200 | 200 | 95,208 | 0.044 | 0.0 mm |
| derived Terra (6.3 mm springs), band 1/256 | 200 | 331 | 0 | 0.042 | 1.9 mm |

- **Today the tide keeps a whole coast awake:** the flat-laid basin is off the sea's slope, so the sea's edge
  pushes it all the time; two thirds of it stepped every step, and it is still 39 cm wrong at the far end.
- **With the head used in `settle`, almost nothing stays awake:** the sea's edge wakes its first column when the
  equilibrium has moved past VSTOP's head (1.4 mm), the body is calm, and `settle` lays all of it to the formula
  at once (one flood fill), back to sleep. The 200 awake are the sea columns themselves. The cost moves from
  stepping into laying: O(body) a few times a minute instead of O(awake) 60 times a second.
- **The re-lay of sleeping bodies** matters only for water the sea's edge does not wake (closed basins, test 2).
  Its band trades re-lays for error: 1/256 block (3.9 mm) is the sensible default (4,600 columns a second here,
  test 2 within 7.8 mm); 1/1024 re-lays the whole basin every second. Its flood fill over every stored column
  each second is in the ms above; the engine should keep a body id a node instead.
- At the derived 6.3 mm the tide barely wakes anything: the water is the formula's to under 2 mm for nothing.

## How to run

    cd prototypes/tide-force
    cargo run --release              # question A's tables
    cargo test --release -- --nocapture
    cargo test --release links_alone -- --ignored --nocapture
    cargo run --release -- bench     # costs (about 8 minutes, most of it "today"'s row)

## Recommendation for liquid.rs

**Step 1, gravity per body.** `Ground` gets `fn g(&self, body: u8) -> f64` (`World::surface_g`, the pull phys
already uses); `Liquids` keeps a small table of `cg(g)` per body index (`Col::body`), used by the speed-up,
`drag_for`/`steady_speed` (a river's friction is chosen from CG, so the generated level only holds with the same
g) and `fall_cap`'s sqrt(2 g h). `moves` and `VSTOP` should be a head threshold, not a speed one, or the dead band
grows as 1/g (1.4 mm at 9.81, 0.7 at 19.62, 7.4 mm on Callisto). Check ACAP's stability: sqrt(g 16 m) dt =
0.30 at 19.62, fine. Test: the dam break, slower on Callisto by sqrt(19.62 / 1.85) = 3.3.

**Step 2, tide as a force.** (a) A general `Tide`: a list of (h_k, dir_k) from the bodies (h from masses,
distances and the body's own pull, as `src/main.rs` does), `base` from the ocean's moment as now; a locked
parent's fixed bulge goes into the bake's sea level, not into the moving sum. (b) eta per stored node in
1/256 units, taken at the tide's second (`tide_epoch`), not per step. (c) `head()` becomes surface - eta in the
links, the wake test and `settle`'s calm test; `settle` lays to the equilibrium, beside the sea to mean + eta per
column (`flatten` today takes one sea neighbour's level: that is the F9 seam). (d) `retide` becomes the re-lay of
sleeping bodies, band 1/256 block; it needs a body id per node so a second's check is not a flood fill. (e) Do
not expect the links to raise a tide; at derived sizes nothing will visibly move until the rows change (Luna at
about 5.5 radii for today's metre), which is the developer's call to make before step 2 is worth building.

## Not verified

- The engine's own `Liquids` was not driven: ceilings, pressure, falls, carried speed, rivers, `forget_matching`,
  the wire's dead band and saves are not in the copy; nor a real coast's shapes (a 2D grid with flat floors only).
- The re-lay's flood fill is the prototype's; the engine would keep body ids.
- No dynamic test of resonance or slosh in a bay (design doc "Open").
- Costs are of the copy, single thread, on a shared PC, not a tick of the server.
- Gravity per body is a recommendation only; no dam-break was run.
