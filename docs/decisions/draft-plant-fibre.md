# NNNN – Plants are cut by hand and give plant fibre

- **Status:** proposed (the numbers and the rules are the developer's call)
- **Date:** 2026-11-17

## Context
The developer (2026-11-14, roadmap "Queued by the developer"): "Grass and the other plants get a hit box so they
can be mined, and give plant fibre (a new item; it is for cord and for hafting tools)." The small plants are
clutter rows of kind `plant` (ADR 0091): crossed painted planes, a pure function of the seed, the column, the
season and the Vegetation setting, turned and shifted in their cell by a hash the shader alone computes.
ADR 0144 aims at pick-ups by their own faces.

## Options considered
How a cut plant stays cut:
1. A taken mark, as a pick-up's (ADR 0082). It is keyed by the cell, and a pick-up and a plant can share a cell.
   It would need a second keyspace and a new save version, and the plant would come back each year.
2. **An edit: the plant's cell written as air.** Chosen. Clutter already hides on a column with an edit in its
   ground cell's chunk (the mesher and `pickup_at` both ask it). The edit goes to every client, into the save and
   to late joiners through the paths every edit takes. There is no wire change and no save change.

How a plant is hit:
1. Its planes: the shader's hashed turn, size and place would have to be repeated on the CPU.
2. **A box**, as the developer said: the middle of the cell, 0.35 m each way of its middle (`PLANT_BOX`), from the
   ground up to the top of its painting's planes. Chosen.

## Decision
- **Aim:** `clutter::thing_at` asks a cell for the pick-up lying there (not taken). With none, it asks for the
  plant growing there at the Vegetation that is drawn: the client's own setting, and on the server the most any
  setting draws (a lower setting's plants are a subset). `clutter::aim_thing` walks the ray as before and tests
  `thing_faces`, which for a plant are its box's six faces. The frame thread still asks only built height tiles.
  The outline goes on the box.
- **Clicks:** left click cuts a plant, whatever is in hand, if what it gives fits. The right click does not pick
  up a plant, so a reed by the water still lets you drink.
- **What can be cut:** only a plant whose row gives an item that exists, on a column that holds no pick-up in any
  season, taken or not. The cut's air edit hides all of a column's clutter, so cutting there would delete a
  pick-up for good: one that regrows next year, or one whose season has not come. A plant that gives nothing
  (wild wheat, flowers, the berry bush) is not aimed at and the server refuses it.
- **Wire:** the same `ToServer::Take { seq, at }`. The server checks reach, finds no pick-up and finds the plant,
  gives what it gives (all of it must fit) and writes the cell as air (an edit sent to every client at once, not through the tick's changes: air stays air,
  so the water is not regapped and nothing is woken; through the changes, a cut among reeds moved water). A
  bot's `pick dx dy dz` cuts a plant the same way.
- **Data:** `gives` is allowed on a plant row (none if left out). Short grass and fern give 1 `plant fibre`;
  tall grass and reeds give 2. Wild wheat, flowers and the berry bush give nothing and cannot be cut. A data test
  holds every `gives` to an item of `data/items.toml`. A new item `plant fibre` comes with a draft model (`data/models/plant fibre.*` by `tools/paint_clutter.py`,
  listed in `data/drafts.txt`).
- **Design:** `crafting.md` names fibre for cord and hafting; no recipe takes it yet.

## Consequences
What stays open:
- A cut plant never grows back. The cut also clears a decoration on that column.
- Each cut is one air edit in the save, with no bound.
- Around a half-season boundary the client's clock can be on one side and the server's on the other. The client
  then aims at a plant the server no longer has, and the Take is refused.
- The server allows the highest Vegetation, so a bot can cut a plant that a client on a lower setting does not
  draw.

Also:
- The box is the same for every plant of a row and its alternates' height; it does not follow the hashed shift
  or the sway.
- Cost: `aim_thing` about 3.7 us a call with plants at the most Vegetation (the engine test's timing, unoptimised
  test profile; 3.1 us before, pick-ups only). Once a frame. No perf-debt row.

## Calls for the developer
- The numbers: 1 from short grass and fern, 2 from tall grass and reeds, none from wild wheat, flowers and the
  berry bush.
- Cut for good (an edit) or growing back next year (a mark, a save change)?
- Should a cut need a tool (a knife cuts faster or gives more), or is a bare hand enough?
- The box outline for a tuft of grass, or nothing until the outline can follow its planes?
