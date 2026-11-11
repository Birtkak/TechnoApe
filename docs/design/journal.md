# The journal: how the ape learns by doing and seeing

- **Status:** design proposal, 2026-11-11. The developer's call is ADR 0027: the ape learns by doing and
  seeing, nothing is locked, a journal fills in from what happens, and there is no research tree. That ADR
  asks for "a journal and a way to notice events in the world"; this is both. Lines marked *taste call*
  are open.

## What it is for, since it locks nothing
A player who knows the game can do everything on a new world from memory, and should. The journal has
three jobs for everyone else:
1. **It says what the world can do**, so nobody needs a wiki to find that wet clay hardens by a fire.
2. **It keeps the numbers the ape measured**: this fire reached 600°, copper wanted 1,085°. Crafting's
   conditions are measured, not declared (`crafting.md`), so the journal is where the measurements stay.
3. **It shows the megaprojects' rules and how far along they are** (progression: milestones are
   measurements).

## An entry has three states

| State | The player sees | It comes from |
|---|---|---|
| unknown | nothing | – |
| **noticed** | a question, in the ape's words: "Green stones in the stream bed. Heavy." | *seeing*: looking at the thing, watching it happen, or another entry pointing at it |
| **known** | the full entry: what it is, what it takes, the numbers from the ape's own try | *doing* it once |

- **Noticed entries are the game's guidance.** The journal opens on the list of open questions, the ones
  that touch a need that is low first (thirsty: the questions about water). There are no quest markers
  and no "next step" arrow: a question is as far as the game pushes.
- A known entry is **exact** (*taste call*, against entries that stay vague): once the ape has fired a
  pot, the entry says the temperature and the time. The mystery is before the first try, not after.
- **Failures teach too.** Clay fired too cold crumbles, and the entry becomes noticed with a line of what
  went wrong: "The pot fell apart. The fire was not hot enough?" A failed process is an event like a
  finished one.
- A new ape starts with a few **instincts** already known: drink, eat, pick up, dig loose earth, strike
  two stones.

## Seeing
- **The world shows processes by itself**, so there is something to see before anyone knows it:
  lightning lights a tree (fire), a lava flow leaves metal beads on an ore outcrop (melting), a stream
  bank shows washed nuggets (washing), mud cracks hard in the sun (drying). `worldgen.md` and the weather
  place these; each is an ordinary event the journal listens to.
- **Watching another ape do it** notices it. A friend, or later an ape of the AI troop, firing pots in
  sight teaches the question. Knowledge then spreads through a server the way it would: by watching.
  Doing still takes the ape's own hands.
- **Looking at a thing** notices it: the block or pile under the crosshair is already known to the game
  each tick for digging, so a lookup of that kind in the journal's rows costs nothing.

## Who holds it (decided: A, ADR 0043)

| Option | For | Against |
|---|---|---|
| **A. Each ape its own** (*chosen*) | a new friend on an old server still discovers; watching others has a point | friends repeat each other's first steps |
| B. One for the whole server | nobody repeats anything | the second player arrives to a finished book |
| C. Each its own, and a known entry can be shown to another ape, who then has it noticed | a way to teach | one more interaction to build; can wait until someone asks |

## The data
`data/journal.toml`, a row an entry, built into the binary and checked like the blocks:

```toml
[[entry]]
name = "fired-clay"
age = "stone"
title = "Fired clay"
question = "The wet pot by the fire has gone hard on one side."
text = "Clay shaped wet, dried, then held in a fire turns to pot. It took {temperature} for {time}."
noticed_by = ["see:process:dry:clay", "look:block:clay"]
known_by = ["do:process:fire:clay"]
points_at = ["charcoal", "crucible"]
```

- A trigger is a small pattern over events: `do:` the ape itself caused it, `see:` it happened in sight,
  `look:` the crosshair rested on it, `need:` a need crossed a line, `have:` an item came into its slots.
- `points_at` notices other entries when this one becomes known: firing clay raises the question of what
  a hotter fire would do.
- The braces are filled from the event's own numbers.
- A **milestone** is an entry with a `measure` instead of `known_by`: the quantity, the threshold and the
  duration of progression's megaproject rules ("a room above 0° for a whole night"). The journal draws
  how far the best try got. One system serves the journal and the megaprojects.
- Text is data, so another language is another file.

## What the engine needs
- **Events out of the tick.** The systems that already decide things (edits, the process runner, vitals,
  the weather, later creatures) each push small records: what, where, who caused it, a few numbers. They
  are a pure result of the tick, in a fixed order, so replays produce the same ones. The effects and the
  sounds can listen to the same list instead of each system calling them.
- **The journal system** runs after them, once a tick: for each event, the rows it can trigger (a table
  from event kind to rows, built at load), then for `see:` the apes within 48 m who face it.
  *shortcut:* no check that something stands between; a wall is forgiven until someone learns smelting
  through a mountain.
- **Per ape:** a state per entry and the numbers of its best try. It goes into the save with the player
  and the state hash, and to its owner only (`ToClient::Journal`), a message per change.
- **Cost:** events are a handful a tick outside a factory, and a factory's machines make no journal
  events once their entry is known to everyone near (the table skips rows nobody can still learn).
  Benchmarked with the soak: 50 bots, the event list and the journal on, against off.

## The book
- A key opens it in the HUD's glass: the open questions first, then pages by age. Noticed entries show
  their question; known ones their text, numbers and a picture of the thing.
- A new question or a learned entry shows as one line over the hotbar for a few seconds and a soft
  sound (a row in `data/sounds.toml`); a setting turns the line off.
- Milestones have their own page with their gauges.

## Tests
- Data alone: every trigger names an event kind, block, item or process that exists; every `points_at`
  exists; **every recipe's process and every item an age needs has an entry**, and every entry can be
  noticed from the instincts by following `points_at` and triggers that the age can cause (the same walk
  as `crafting.md`'s reachability test).
- A scripted bot plays the starter's first steps and its journal must hold the expected entries, known.
- Two bots, one fires clay while the other faces it, then faces away: noticed, then not.
- Replay: the journal is in the state hash.

## Order of building
1. The event list, with effects and sounds moved onto it (no new behaviour: the views and the soak hash
   say so).
2. The journal's data, state, message and save, with `look:` and `have:` triggers: it already works for
   blocks and items before any process exists.
3. The book.
4. `do:` and `see:` with the process runner (`crafting.md`), and the starter's entries written.
5. Milestones with the first megaproject (Home, with `needs.md`'s warmth).
