# ADR 0058: Wild animals: forest, water and cave species

Status: accepted (2026-10-25)

## Context
Part E of the 2026-10-25 split: more animals. Until now: sheep, pigs, chickens (ADR 0022) and bears
(ADR 0027), all walkers on grass or snow. Developer calls (2026-10-25): forest and plains wildlife
(deer, boar, rabbit, fox), water edge (duck, frog, crab, fish), underground (bats, cave spiders); later
wolves too (part F tames them into a dog); who bites: wolves (some packs), boars (when hurt), cave
spiders (always). The model sheet was approved as drawn.

## Decision
1. **Eleven species in `data/mobs.json`**, each with a display name, drops, food, spawn biomes and
   sounds: deer (antlers drop now and then), boar, rabbit, fox (hunts rabbits, chickens, ducks, frogs;
   never the player), wolf (packs of 3-5, hunt deer, sheep, rabbits, pigs, boar; 30 % also hunt the
   player), duck (lays eggs), frog, crab, fish (`raw_fish`, cooked at a firepit or furnace), bat, cave
   spider. New items: raw and cooked fish, fur.
2. **New data keys** (`WildMoves`): `habitat` land / shore / water / cave; `moves` walk / hop / swim
   / fly / paddle; `skittish_m` (flee a player this close); `defends` (a grazer with predator rules
   for its attacker only: the boar); `predator.pack` (join a pack mate's prey); `ground` (crabs on
   sand). Fish swim in 3D through water and flop and suffocate on land; bats flutter through cave
   air; hoppers jump along; ducks may paddle out onto water.
3. **Spawning**: land herds roll as before (now on their `ground`); each spawn cell also rolls for a
   water group (fish in water at least 0.5 m deep, ducks and frogs on its banks) and for a cave group
   (bats or spiders in a cave pocket 6-40 m below the surface, once that ground is loaded).
4. **Models** in `scripts/mobs/wild_models.gd`, MobModel's cells (3.1 cm) with faces on even cells so
   part C's half-voxel merge and 8-shade ramps (part C's branch) apply unchanged; colours
   appended to `MobModel.PALETTE`. MobModel gains a generic leg swing (0, 2, 4, 6 or 8 legs), tails,
   and asks `WildModels` for layout, cells and extra animation. Mob and MobManager gain small hooks.
5. **Sounds**: CC0 BigSoundBank recordings (ducks 0276, frog 0819, bats 0458, sika deer 3616, dogs
   1060 / 2953 / 2955 / 0682 for wolves and foxes, steam hiss 0227 for spiders, broken plates
   1643/1644/1646 for pots), cut by `tools/make_sounds.py`; boars reuse the pig grunts pitched down,
   crabs gravel clicks, fish small splashes.

## Consequences
- Herds of the new species fill the same `max_active` budget; cave and water rolls add voxel reads
  per new spawn cell (a few hundred, once per cell, spread over the 1 s scan).
- No pathfinding: fliers and swimmers pick reachable-looking spots and give up on a bump.
- Taming wolves, fishing gear (weirs, traps, hooks) and fur clothing belong to part F.
