# Clutter and pick-ups: what lies on the ground

- **Status:** design, 2026-11-12; **built 2026-11-13 (ADR 0082)**: clutter computed from the seed and pick-ups
  taken by looking and clicking (bots' `pick`). Open items below are still open. It is step 7 of `see-through-and-boxes.md`, split in two:
  **7a clutter** (decoration) needs that document's step 6 (still models in the chunk mesh); **7b pick-ups**
  also needs items to exist (`items.md`, a proposal: today only blocks can be owned).
- **The developer's calls (2026-11-12):** living things regrow, stones do not; a pick-up is taken by looking
  at it and clicking; decoration does nothing when walked through, and building on its spot removes it.
- **Reference:** Vintage Story lays loose stones, flint and sticks on the ground at world generation and
  they never return; Valheim's pickables return on a timer. We take the first for stone and a seasonal
  return, with no timer, for what lives.
- **Why it matters:** the first ten minutes are "drinks, eats what grows, picks up sticks, flint, loose
  stones" with bare hands that "only take loose things" (`crafting.md`, `progression.md`). Without pick-ups
  the first age has nothing to start from.

## One idea: the ground's small things are computed, not stored
Whether a thing lies on a column, which thing, which of its shapes and how it is turned is a **pure function
of the seed, the column and the season**. The server stores nothing until a player touches one; the client
computes the same answer and draws it. A world of ten million pebbles costs no memory, no tick and no bytes
on the wire. Only what was **taken** is kept.

This is the still plants' method (`World::candidate`: one hashed spot per 4 x 4 columns) made finer: every
column that has open ground is asked.

## The data: `data/clutter.toml`
One row per kind of thing, with a schema and tests like every data file:

| Field | Meaning |
|---|---|
| `name` | unique; also the model's name (`data/models/<name>.toml`, its alternates `.1` to `.4`) |
| `kind` | `"decoration"` or `"pickup"` |
| `gives` | pick-ups only: the item and how many |
| `on` | the ground blocks it may lie on (grass, dirt, sand, gravel, rock ...) |
| `near` | optional: `"tree"` (within 3 columns of a standing trunk), `"water"` (a river or lake bank), `"rough"` (broken ground, the bake's `rough`), `"open"` (none of these) |
| `season` | optional: the seasons it is there (mushrooms: autumn; flowers: spring, summer) |
| `regrow` | pick-ups only: absent (never, stone) or `"year"` (a new crop each year, at the start of its first season) |

A biome row in `data/biomes.toml` gains `clutter = [[name, per 100 columns], ...]`, as it lists its plants.
The hash picks at most one thing per column; a pick-up wins over decoration on the same column.

Never: under the sea, in a river or lake, on a face edge's last columns (as plants), on a column whose top
block is not the generated one.

A first list, to be painted as box models (decoration, then pick-ups):
grass tuft, fern, dead leaves, twig litter, moss patch, small flowers, pebbles; **stick** (near trees,
regrows), **loose stone** (rough ground and banks, never), **flint** (banks and gravel, never), **mushroom**
(near trees, autumn, regrows), **berries** on a low bush (open ground, summer, regrows).

## Clutter (7a): decoration
- Drawn on the nearest chunks only, as part of the chunk's mesh (the spike: one per 4 columns costs 0.2 ms
  of GPU and nothing on the frame thread; ADR 0068). No collision, no shadow rays, no sound.
- The Graphics setting **Clutter** (off, few, normal, many) thins decoration by a threshold on the same
  hash, so "few" is a stable subset of "many" and nothing shuffles when the setting changes.
- A season's change re-meshes the near chunks on the workers, spread over frames.
- **Removed by building:** a block placed in its cell, or the ground under it dug, and it is gone, because a
  column with an edit has no decoration. Nothing new is stored for this; the edit that was made anyway is
  the record. It does not come back if the block is removed again.
- The server never computes decoration.

## Pick-ups (7b)
A pick-up is clutter the server answers for.

- **Pointing:** the aim ray (`edit::aim`) also tests the small box of a pick-up in the air cells it crosses
  (pick-ups only, never decoration). The thing under the crosshair is outlined and named.
- **Taking:** a click (the dig button) takes it at once; bare hands take loose things. The client sends
  `Take { cell }`; the server checks reach, that the function says a thing lies there now, that it is not
  taken, and that the inventory has room, then gives the item and tells the clients near it. Two apes
  clicking the same stick: the first gets it, the second is told no.
- **What is stored:** a **taken mark** per cell with the crop it was taken in (a small number: the count of
  years since the world began, 0 for things that never return). A thing is there when the function says so
  for the current season **and** no mark of this crop exists. So regrowth needs no timer and no tick: the
  year turns, the old mark no longer matches, the mushroom is back. Marks of past crops are dropped when the
  world is saved; a stone's mark stays for good.
- **Marks travel and are saved like edits** (per chunk, to the clients that have the chunk; one more list in
  the save, ADR 0016), and a mark re-meshes its chunk as an edit does.
- **A felled tree drops no more sticks:** `near = "tree"` asks whether the trunk still stands, so the sticks
  under a tree that was cut do not return next year, and a planted tree (W7, living plants) will make its own.
- **Building over one:** placing a block in a pick-up's cell, or digging the ground under it, takes it for
  that ape first; with no room in the inventory it is left as a pile (`items.md`).
- **Finite stone is counted:** `technoape-server report=1` (ADR 0063) gains the loose things per biome, and a
  data test holds the opening to its promise on sampled spawn areas: within 30 m of a forest spawn at least
  10 sticks and 5 loose stones, flint within 150 m. The numbers are the first guess to play-test.
- **Bots and the soak:** `take` joins the bot's commands; the soak's conservation law counts taken things
  into the inventories they went to.

### Pick-ups and piles are two things
`items.md` proposes that **walking over a pile picks it up**. That stays for piles: a pile is something an ape
dropped, set down or dug loose, and wanting it back is the likely case. What the world laid is taken by
hand, because the ape walks over the forest floor all day and would fill its nine slots with stones. One
line for the player: *what you dropped comes to you, what the world laid you pick.*

## Cost and checks
- **Server:** nothing asleep; one hash and a few lookups per `Take`. A benchmark case: 50 bots taking things
  as fast as the reach allows.
- **Client, CPU first:** the mesher asks the function per open column of a near chunk. The spike's scan cost
  57 microseconds a chunk before any tuning; the budget for 7a is under 30. `near = "tree"` looks at the
  neighbouring plant spots only for the rows that ask.
- **Determinism:** a test that the server's and a client's function agree on every column of sampled chunks,
  in all four seasons; a replay test that takes, saves, loads and finds the same ground.
- The fixed viewpoints change (the ground gains clutter): looked at, then accepted, once.

## Left out on purpose
Trampled paths; clutter under water (reeds and river pebbles can come with W7); snow hiding clutter (snow is
not built, ADR 0061); creatures eating what grows; a Studio page for `clutter.toml` (the file reloads live
like the others; the Studio's "preview as ground" from `see-through-and-boxes.md` shows a model scattered).

## Build order
1. **7a:** `data/clutter.toml` with decoration rows, the function in the engine with its agreement test, the
   mesher's call, the Clutter setting, the views accepted. Needs box models and still models (steps 4 to 6).
2. **Items** as far as an item that is not a block can be owned (`items.md`'s first steps).
3. **7b:** pick-up rows, the aim test, `Take`, taken marks in the wire and the save, the report and the
   opening's data test, the bot's `take`.
4. Regrowth by season and the felled-tree rule.

## Open questions for the developer
- **Which button takes?** The default is the dig button (one click, no hold). A separate use key would also
  serve chests and machines later; `items.md` has the screen opened by a key already.
- **Do berries and mushrooms feed?** Assumed yes (`needs.md`); how much is that document's call.
