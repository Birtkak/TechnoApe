# River reach spike: a reach at rest is generated, a touched one is live

**Question** (worldgen-plan W5, `docs/design/rivers-and-tides.md`, "Rivers"): can a river reach be generated water
that costs nothing and is stored nowhere, like ADR 0020's sea, become live water when touched, and go back to
generated water afterwards, with no visible step, every unit counted, and a dam needing no rule of its own?

**Answer: yes**, with five changes to `liquid.rs` (copied here as `src/liquid.rs`, then built into the engine):

1. **A river column's own level.** The open edge of the stored water is held at the stored column's *own*
   generated level, not its unstored neighbour's: a river slopes, so the two differ by the reach's fall a column.
   Each column books against an account (`Ground::reach`): the sea, or the reach whose water it is.
2. **Three kinds of open end** at an unstored wet neighbour:
   - downstream (its level is lower): the edge column is held at its level after the step's flow, so it takes
     whatever arrives (a dam-break wave leaves there) and gives back what is missing;
   - level (the sea, a lake, across a river): held while within 3 cm, else the patch grows (ADR 0020 as it was);
   - upstream: it **pours in the reach's flow**, whatever the edge holds, booked against the reach; if the edge
     stands over its level by 3 cm (a dam's pond), the patch grows upstream instead.
3. **Friction that makes generation the live steady state.** A river column's friction is chosen from the reach's
   flow per column of width `q`, the surface's fall a column and the column's generated depth (`drag_for`),
   proportional to the depth. Every link then loses the same head per column whatever the bed's 1-block steps do,
   so live water carrying `q` stands exactly at the generated level. A woken river column starts with the steady
   speed on its links, and gathers its runoff (`lat`) while live. Water does not carry its speed from link to link
   in a river column (that term held a fall's patch 1/64 over generation for good).
4. **Falls.** A link across a generated drop (more than half a block, or 4 times the reach's fall a column)
   speeds water up on the reach's own fall plus the lip's height over its level, not on the drop: the lip
   passes the reach's flow at its generated depth.
5. **Forgetting a moving patch.** Every 15 steps, a body of water whose every column is within the dead band
   (1/64 block) of generation is forgotten, moving or not, the difference to each column's account. The dry
   shore around it goes too (one ring after another).

Generated river levels are smooth (bed + depth, to the unit). A level stepped to 1/8 block would stand up to 1/16
off the live water's smooth slope, and a nudge off a block's top took from the fall the open end's feed reads.

## Measured (`cargo run --release` here; Ryzen 5 5600X; a straight channel 6 columns wide, 400 long; its flow
grows from 4 to 8 blocks/s along it; bed falls by 1-block steps under a smooth surface 1.5 blocks deep)

| case | answer |
|---|---|
| Q1: step at wake or sleep | none by construction: a woken column holds its generated water exactly; a patch is forgotten only within 1/64 block |
| two buckets in the reach, slope 0.005 / 0.02 / 0.05 / 0.1 | live up to 0.11 / 0.08 / 0.04 / 0.03 blocks off generation, forgotten after 4.0 / 4.5 / 3.5 / 3.8 s; 21 columns at most, 3-5 k node-steps |
| the same at 1 block deep | forgotten after 5.2 s |
| Q5: six buckets on both sides of a 3-block fall | forgotten after 7.0 s, 32 columns at most, 10 k node-steps |
| Q2: a dam 2 blocks over the river, 120 s, then broken | ledger exact every step (reach water + stored - generated + the accounts - runoff + what reached the sea); the pond rose 1.7 blocks and backed up about 190 columns, 1,177 stored; the reach below received 0.6 blocks/s instead of 4 |
| Q3: the broken dam's wave | live 214.5 s, at most 1,186 columns, 11.4 M node-steps (53 k a second: about 0.1 ms a tick at ADR 0020's 117 ns a node-step); the reach below received up to 5.6 blocks/s while the pond emptied |
| a pillar in the river (a bridge) | after 3 min 1 column stored (the pillar's), asleep, 0 node-steps a second: the river flows past it as generated water |
| Q4: a wheel reads the reach's speed | 1.8 ns a read, 0 columns stored, 0 node-steps (asserted) |
| the reach graph, 4,000 reaches | 15 us a tick (once a second) |

**Q3, how long a wave stays live:** no timer is needed. A wave is live until its patch matches generation again;
the costliest case measured (a whole pond emptying) was 3.5 minutes at about 0.1 ms a tick.

## Known limits (carried into the engine's ADR)
- Generated levels follow the reach at rest (the bake's mean flow). The reach's water number changes with dams
  (the reach below a dam got 15 % of its flow), but its drawn level does not: below a dam the river looks full.
  Live levels need the reaches' levels sent to clients and chunks remeshed; that comes with live rain (W3).
- A dam's pond is fed the reach's flow and nothing more: it does not spill over the banks in this spike (the banks
  were high). Over low banks it floods out into a live patch that stays until the dam goes.
- Where a reach's flow changes at once (a confluence), a patch spanning it is fed by both open ends, so it matches;
  within a reach the flow grows by `lat` a column.
