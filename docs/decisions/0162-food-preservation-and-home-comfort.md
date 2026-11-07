# ADR 0162: Food preservation stations and home comfort

Status: accepted (2026-11-07)

## Context
Spoilage (ADR 0040), the drying rack with a smoke mode, salted meat and the root cellar (ADR 0059, 0061)
already exist. Missing: a station that smokes in bulk, salt that does not cost fuel, salted fish, and any
reward for building a proper bedroom.

## Decision
1. **One class, three pieces.** `RackEntity.SPECS` runs the drying rack (4 hooks), the **smokehouse** (12 hooks,
   recipe hours / 1.5, smokes while a lit firepit stands beside or under it, like the rack) and the **salt pan**
   (2 slots, recipe mode `evaporate`). Rack recipes gain the mode `evaporate`; the data is `stone_age_recipes.json`:
   `pan_evaporate_brine` (pot of brine -> pot of salt in 8 game hours, only with open sky in daylight, `StoneAge._sun_on`),
   which the existing recipe turns into four salt. Brine comes from dipping the cooking pot in the sea, as before; the
   pan saves the firewood the firepit boil needs. No new materials (work pieces).
2. **Salted fish** (`salted_fish`, spoil 80 days, meat 2) beside salted meat (90 days). Each preserved food keeps its own
   `spoil_days` and nutrition in the data files; the rate maths is unchanged (`Survival.age_inventory`).
3. **Cellar**: unchanged and tested (`Seasons.container_rate`: a closed room 1.5 m down spoils at 0.3 x the 8 C rate).
4. **Home comfort** (`Comfort`, pure maths + `of_bed`): 0..100 points = enclosure 40 (eight horizontal rays hitting a wall within
   4 m = 70 %, a roof within 3.5 m = 30 %; ~290 voxel reads) + light 20 (a lit entity or lantern within 6 m) + warmth 20
   (`Survival._entity_heat` at the bed, full at 5 = a firepit at 2 m) + furniture 20 (pieces within 6 m, full at 4).
   Tiers Bare / Plain / Cosy (>= 45) / Comfortable (>= 60) / Luxurious (>= 70). Looking at a bed shows the rating.
5. **Rested** (`Survival.buffs["rested"]`, amount = points / 100): 45+ points earn it by sleeping through the night, or
   a nap (daytime click, half as long). Lasts `comfort_rested_s` (900 s) x amount. At amount 1: hunger drains 35 % slower,
   health regenerates twice as fast, walking 5 % faster. All tunables in `settings.gd`.

## Consequences
- Rays count a lamp or fire outside the walls if it is within 6 m (no line-of-sight test): cheap, slightly generous.
- A room is rated when you look at the bed or sleep in it; nothing is cached or ticked.
- The progression walk now needs a smokehouse for smoke recipes and a salt pan for the evaporate recipe.
