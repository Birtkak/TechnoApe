# ADR 0100: The steam age, stage 3 transport: hoppers, chutes, belts, rails and the minecart

Status: accepted (2026-10-28)

## Context
ADR 0096 planned stage 3 of the steam age as "ore in one end, ingots out the other" and hauling
from far veins. ADR 0099 built stage 2 and the turbine; the developer asked for the transport next.

## Decision
1. **One loading API for every piece** (`Transport`, static): `give(e, stack, items)` puts one item into
   a hopper, belt, chest, firepit-family station (forge, bloomery, blast furnace, boiler fuel) or a steam
   sieve or crusher, through each one's own `use`/`accept`/`inv`; `pull(e)` takes one finished item out
   (a firepit's output, a sieve's or crusher's finds, a hammer's finished piece, a chest's or hopper's
   first stack). Facings are quarter turns from -z in `WorkPalette.rotate_y`'s sense (`Transport.DIRS`),
   laid the way the player looks (`facing_from_yaw`); a wrench turns them like a boiler.
2. **The hopper** (`HopperEntity`, SLOTS 5): every `hopper_s` it pulls one item from the piece above and
   pushes one into the piece it faces, or under it. The **chute** is the same entity with `item` "chute"
   (planks and a rod, cheap): it also pulls from behind and only drops below, the way down a ledge.
   A right click loads one item, an empty hand empties it.
3. **The belt** (`BeltEntity`): a half block carrying up to CARRY items, GAP apart, one block in `belt_s`
   the way it faces; the head item goes into whatever is at its end (`Transport.give`), and waits when
   that refuses (a sieve already sifting something else). The items are drawn riding along (the laid
   items of the view).
4. **Rails and the minecart** (`RailEntity`, `axis` like a pipe, flat only): `next_of` joins the rail
   straight on or round either corner. The minecart is an item; set on a rail it becomes the rail's
   `cart` flag (drawn as an iron tub on wheels). A right click rides it: `Player.ride` holds the rail,
   the direction and the position along it, `_ride` rolls it at `minecart_speed_m` with forward and
   back, moves the cart flag from rail to rail, stops at the end of the line, and sneaking gets off.
   Sneak and right click takes the cart back.
   **Slopes** (developer, 2026-10-28): a rail's `rise` is the block step it climbs; the wrench cycles flat,
   up one way, up the other, then the other axis. `next_of` goes up the slope a rail climbs, down onto a
   slope whose top meets this level, and only turns corners on the flat; `height_on` lifts a cart along it.
   **The chest cart** (a minecart and a chest; `cart` is "minecart" or "chest_cart", CARGO 9 slots): items
   in hand go in, hoppers above load it and hoppers below empty it while it stands, and an empty hand pushes
   it: it rolls by itself (`rolling`, `t`, the rail's `tick`) at `minecart_speed_m`, cart state moving from
   rail to rail, until the line ends. A push at each end and a hopper over each end is the ore train.
5. `/kit steam` carries hoppers, chutes, belts, rails, a minecart and two chests; sheet kinds and
   sprites added.

## Consequences
- `test_transport.gd`: a chest-fed hopper over two belts into a hopper, a chute into a steam sieve and a
  belt that waits, rails round a corner with the cart flag, saves.
- Not built: hoppers that catch dropped items, belts that climb, carts that keep rolling over the player,
  tanks and turbines as hopper targets, any in-game check of riding (unit tests and a headless boot only).
- Hoppers and belts tick in `BlockEntities`: long lines are cheap (one item a second each), but a belt
  with items redraws its view every four ticks.
