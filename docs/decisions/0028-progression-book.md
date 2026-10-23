# 0028 – Progression book: FTB-style quests, Flint Age first

- **Status:** accepted
- **Date:** 2026-10-23

## Context
The Vintage Story early game (ADR 0023) has many steps that are not obvious:
rock drops nothing, knapping and clay forming happen on the ground, a pit
kiln wants grass then sticks then fire. The developer asked on 2026-10-23
for "a progression book in the top left corner when you open your
inventory ... like quest books from Minecraft. For now add only flint age
and let it end with making your first copper tool."

## Options considered
1. **Advancements** (vanilla Minecraft): a tree of toasts, little text.
   Says what happened, not what to do next.
2. **A guide book** (Patchouli / Vintage Story handbook): pages of text.
   Good reference, no sense of progress.
3. **Quest book** (FTB Quests): chapters of quests drawn as a dependency
   graph; each quest has text and tasks with checkmarks. Chosen: it is what
   the developer named, and it shows both the road and where you are on it.

## Decision
Option 3, data-driven:

- **Data:** `game/data/quests.json`: chapters, quests with `pos` on a page
  grid, `deps`, `tasks`, optional `mode: "any"` and `final`. Task types:
  `item` (hold `count` of an id, a `#tag` or any id in `items`) and `event`
  (`fired`: a pit kiln finished firing that item). Ids are saved, so they
  are append-only.
- **Logic:** `QuestBook` (`scripts/quests/`, headless). Item tasks are
  counted whenever the player's inventory changes; events come from
  `GroundEntity.fired` through `WorkSystem`. Progress **latches** (best count
  seen) and counts while a quest is still locked, so nothing has to be
  gathered twice. A quest completes when its tasks are done and its deps are
  complete; completions cascade.
- **UI:** a book button top left of the inventory screen (JEI bookmarks
  move below it) opens `QuestBookView` over the screen: chapters left, the
  graph in the middle (done green with a check, available pulsing gold with
  "!", locked dark), the selected quest's text and tasks right. Clicking a
  task's item opens its recipes in the browser. `QuestToast` shows a
  Minecraft-style toast per completion, and chat logs it.
- **Saving:** per world, `user://worlds/quests_<seed>_g<gen>.json`, with the
  other block-entity files.
- **Flint Age:** 20 quests from loose flint to any copper tool. The axe,
  timber, workbench and sieve branch is optional (sticks fuel a firepit too,
  and surface copper stones make the sieve unnecessary).
- **Tooling:** `/quests [list|complete <id|all>|reset]`; debug args
  `quest_book=1|click`, `quest_select=<id>`, `quests_done=a,b|all`;
  `quests` in the stats JSON.

## Consequences
- No rewards yet. FTB books hand out loot on completion; whether ours
  should is a taste call for later.
- New ages (bronze, iron, electricity) are new chapters in the same file;
  the sidebar already lists chapters. A chapter wider than the canvas will
  need panning (it scales down to fit today).
- Tasks only see the inventory: an item crafted and used up straight from
  the cursor before it lands still counts (the cursor is included), but
  something placed without ever being held would not. New events cover
  such steps when they come up.
