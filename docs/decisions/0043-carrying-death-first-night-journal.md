# 0043 – Nine slots and packs, death drops a pile, a night that hurts, a journal per ape

- **Status:** accepted
- **Date:** 2026-11-11 (developer's calls in a design session; nothing built yet)

## Context
Three designs written the same day (`docs/design/items.md`, `needs.md`, `journal.md`) each turned on a
taste call: how the ape carries, what dying costs, how deadly the first night is, and whose the journal
is. Numbers 0041 and 0042 are taken by ADRs on branches not yet merged.

## Options considered
Each was put to the developer with a recommendation; the tables are in the three designs.
- **Carrying:** the hotbar's 9 slots and rows earned by packs; 36 slots from the start, as Minecraft; a
  weight limit.
- **Death:** drop everything where the ape died and wake at Home with full needs; keep everything (as
  built in ADR 0040); drop everything and wake weak.
- **The first night**, bare and in the open on a temperate plain: the ape lives, hurt and hungry; it
  dies; it is only uncomfortable.
- **The journal:** each ape's own; one per server; own, with entries that can be shown.

## Decision
The developer chose the recommendation in all four.
- **The ape starts with the hotbar's 9 slots.** What it wears adds rows: a woven basket 9, a hide pack
  18, a frame pack 27. Stacks are Minecraft's (64 by default).
- **Death drops what the ape carried as a pile where it died**, marked by a waypoint; nothing despawns.
  The ape wakes at its sleeping place, else at Terra's spawn, with full health and needs.
- **The first night in the open hurts and does not kill** on a temperate plain: cold drains hunger faster
  and costs some health. A fire or a cave makes it free. The poles and high ranges kill at any hour.
- **Each ape has its own journal.** Watching another ape do a thing gives the question, not the answer.

## Consequences
- The inventory moves from counts per block to slots (`items.md` step 1) before any pack exists; until a
  basket can be made, survival has nine slots, so the basket must be an early and short craft.
- ADR 0040's "the inventory is kept" on respawn ends when piles exist (`items.md` step 2, `needs.md`
  step 3). A sleeping place becomes a thing to build and claim.
- Warmth is tuned against one sentence, and a scripted bot's first night is its test (`needs.md`, Tests).
  The air spike's day and night swing answers to it.
- The journal's state is per player in the save and the state hash; seeing needs events with a place and
  a cause (`journal.md`).

## Added 2026-11-11: four more calls
Asked the four smaller questions the designs had left on a recommendation, the developer answered:
"1: ground 2: nothing 3: Protein, vegetables / fruit together, dairy and carbohydrates and 4 u can
decide". Three of them go against the recommendation.
- **A dug block falls to the ground** as a pile and is picked up by walking over it, as in Minecraft
  (recommended: straight into the slots). Piles therefore come second in `items.md`'s build order, before
  the inventory screen, and a dug shape gathers its drops into few piles.
- **A worn-out tool leaves nothing** (recommended: its head as scrap). The metal goes to a "worn away" row
  of the body's ledger, so tools drain a finite planet's metal.
- **Food has four groups:** protein, vegetables and fruit together, dairy, carbohydrates (recommended:
  none). *Claude's reading, not the developer's word:* how they act. `needs.md` proposes four marks beside
  the hunger bar and a reward for variety with no penalty, since dairy needs herded animals; whether a
  missing group should ever hurt is open (`open-questions.md`).
- **A known journal entry is exact**, with the numbers of the ape's own try: left to Claude, who kept the
  recommendation.
