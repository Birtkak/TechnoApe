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
- ADR 0040's "the inventory is kept" on respawn ends when piles exist (`items.md` step 3, `needs.md`
  step 3). A sleeping place becomes a thing to build and claim.
- Warmth is tuned against one sentence, and a scripted bot's first night is its test (`needs.md`, Tests).
  The air spike's day and night swing answers to it.
- The journal's state is per player in the save and the state hash; seeing needs events with a place and
  a cause (`journal.md`).
- Still open (`open-questions.md`): where a dug block goes, what a worn-out tool leaves, food groups, and
  whether a known entry is exact. The designs carry a recommendation for each as their default.
