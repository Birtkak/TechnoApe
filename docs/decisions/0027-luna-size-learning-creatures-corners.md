# 0027 – Luna at 1 km, learning by doing, hostile creatures, made things at the corners

- **Status:** accepted
- **Date:** 2026-11-10 (developer's calls in a design session)

## Context
Four questions blocked the designs written after ADR 0026: Luna's size (the ore budget and Luna's world
generation), how an ape learns (crafting), what lives on Terra (living plants, survival), and what the
cube corners hold (ADR 0003 left it open; world generation places them).

## Options considered
Each with a recommendation put to the developer; the full list is in `docs/design/open-questions.md`.
- **Luna:** 177 m as today; about 500 m; about 1 km.
- **Learning:** by doing and seeing; a research tree; no in-game knowledge.
- **Creatures:** animals only; animals and hostile creatures; none for now.
- **Corners:** natural wonders; something made; nothing special.

## Decision
- **Luna is about 1 km in radius** (N = 3072, 1,061 m), a quarter of Terra.
- **The ape learns by doing and seeing.** Nothing is locked; a journal fills in from what happens.
- **Terra has animals and hostile creatures.**
- **The 8 corners hold something made.**

The last two differ from the recommendations (animals only; natural wonders): the developer wants danger
and a story in the world.

## Consequences
- Luna's orbit must move out or it fills the sky (proposal: 30 km, 4° across); ADR 0022's sky numbers,
  the eclipse band, the `orbit` benchmark's trip and Luna's benchmark rows change with it. Luna's world
  generation (`worldgen-plan.md` W6) and its ore budget get room.
- No research tree is built. Crafting needs a journal and a way to notice events in the world.
- Combat, weapons and base defence become content, with their own design to come: what the creatures
  are, whether they attack bases, how they find their way on a sphere, and their cost in the server tick.
- The corners need lore and structures the generator places, visible from orbit, on a planet that can be
  mined out. `open-questions.md` proposes tying the creatures and the exotic resource to them.
