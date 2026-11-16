# TechnoApe – Lore: the Indigenti and the uplifted ape

- **Status:** living document, started 2026-11-16 in a brainstorm with the developer. The story below is the
  developer's; lines marked *proposal* are Claude's and open. Nothing here is built. It answers the open
  questions of ADR 0027 (the hostile creatures and the exotic resource; its corners call is dropped, ADR 0106) and
  would change `progression.md` (the fuel ages and two more systems); that rewrite waits until the story is settled.

## The story in one paragraph
The **Indigenti**, an advanced species living at the second-closest star to Sol, cannot leave home: the fuel
for interstellar travel cannot be found, only made, and their system cannot make it. So they sent a
**self-replicating machine swarm** to Terra, slowly, to build a **monument**. The first animal to touch it is
bound to it: it gains **sentience and an unstoppable hunger for advancement**. That animal is the player's ape.
The Indigenti wait for the species to become spacefaring and to learn to make the fuel; then they can **take
over its free will** and make it their bottom-line supplier.

## Setting: three systems
| # | System | Role |
|---|---|---|
| 1 | **Sol** (Terra, Luna, the planets) | where the game starts; the bulk of the early and mid game |
| 2 | **Alpha Centauri** | the nearest star; extra content; playtime to the moon is cut to make room |
| 3 | **The Indigenti's star** (name redacted, the developer's) | their home; the end of the game |

## The fuel ladder
Each rung is a real kind of fuel, and each opens a new reach.

| Rung | Fuel | Reach | Notes |
|---|---|---|---|
| 1 | **Chemical** (hydrogen and oxygen, methane) | Luna and the nearer planets | EV in `progression.md` |
| 2 | **Fusion** (He-3 from Luna) | the outer solar system | the "exotic" step; He-3 and fusion are already on Luna in `worldgen.md` |
| 3 | **Antimatter** | Alpha Centauri, then the Indigenti's star | the last fuel; **can only be made, never found** |

## Why antimatter, and why the Indigenti cannot make it
Antimatter is stored power: real accelerators turn far less than a millionth of their input into it. A ship's
worth takes the whole grid of a planet-scale civilization, a collider ring and magnetic containment.

The Indigenti cannot make it, for three stacked reasons:
1. **Power.** Their star is a dim red dwarf and a flare star; flares wreck collectors. They cannot harvest the
   power safely at the scale needed.
2. **Materials.** Red dwarfs of that kind are metal-poor: small planets, no bulk metal, no superconductors or
   magnet material. Sol and Luna have them.
3. **A past failure** (*proposal*): they tried. A containment accident destroyed their industrial world. They
   know what is needed and cannot risk it again. The wreck is a late-game place, and shows what a failed
   collider does.

So they sent the swarm: slow, patient, cheap, and the one thing they could afford.

## What it explains in the existing design
- **The ape's drive.** "Every need starts as a chore and ends as a factory" is the hunger the monument gave
  it; the ages and their megaprojects are what the hunger does.
- **The monument.** The swarm's work needs a place. ADR 0106 dropped the 8 cube-corner landmarks, so where
  the first-touch monument and any other structures stand is open (`open-questions.md`).
- **Hostile creatures.** *Proposal:* what the swarm leaves or releases, not wild predators. They scale with
  the ape's reach, and have a reason to care about noise, smoke and power. Open until the monument's place
  is decided.
- **The exotic resource.** The collider ring is the fourth ring megaproject (after railway and power ring);
  the refinery of `progression.md` becomes the collider complex. Hydrogen comes from water, so it drains the
  seas; its power comes from Luna's fusion. *Proposal:* the raw material found only off the planet becomes
  the superconductor or magnet material the collider cannot run without, so the moon base stays necessary.
- **"Quantum liquid" in pipes.** *Proposal:* the ape's name for it. Antimatter cannot touch a wall, so its
  "pipes" are powered containment lines. If the power fails it annihilates and takes the facility with it.
  This is the "decays unless powered" rule the docs were looking for.

## Open questions
- **When does the player learn the truth?** *Proposal:* through the journal (`journal.md`), one find at a
  time, as noticed entries, never as a cutscene; the ape's own words.
- **What can the player do about it?** **Left open on purpose** (the developer, 2026-11-16): the ending is
  decided later, once the game shows what it has become. Nothing built until then may assume one ending; the
  story's beats up to the third system must work with any of them.
- **Multiplayer:** the first to touch is bound. Are friends' apes bound too, or different? Open.
- **Where the swarm's work stands**, on Terra and on Luna: open (the corner landmarks are dropped, ADR 0106).
- **Does the swarm still run?** Is it still replicating, and is that what the hostile creatures are? Open.
- **The Indigenti themselves:** how they look, how they speak, whether they are ever met. Open.
- **Scope:** Alpha Centauri and the Indigenti's star as playable systems is a very large addition to the
  roadmap; the planet-sized content of `progression.md` (Starter to HV) stays the first target.
