# 0167 – Players see each other's look, colour and place

- **Status:** accepted
- **Date:** 2026-11-08 (bug reports 2026-11-08_15-46-01 "cannot see what other players are holding",
  16-05-40 "armor also not visible", 15-57-27 "see other players their stains", 15-57-41 "tp to players and see
  them on the map"; the developer: "different coloured monkeys and clothes, a random not used one to each player
  joining so it's easier to see who is who")

## Context
ADR 0145 relays a 20 Hz state packet per player (position, yaw, pitch, speed, flying, pet). Everything else
about how a player looks stayed local: every avatar was the same white-coated ape with empty hands.

## Decision
- **A look packet beside the state.** `Net._s_look(d)` (reliable) carries `{"h": held item id, "w": {slot: id},
  "s": {stain kind: amount in tenths}}`, built by `RemotePlayers.look_of`. The client sends it only when it
  changes (checked at the state tick), so it costs nothing while a player stands still. The server stamps the
  sender's colour into it (`"c"`), keeps the last one per peer and gives a joiner every kept look; a look over
  2 KB is dropped. Receivers ignore item ids their registry does not know.
- **Drawing it.** The avatar holds the item in its right arm (sprite, block or card, as the viewmodel picks),
  the arm lifted forward; worn clothing goes through `ApeModel.set_worn` as for the local body; stains tint the
  body with the player's own stain overlay material (no new pipeline to compile).
- **Ape colours.** `ApeModel.VARIANTS`: eight palette overrides of fur and coat (0 = the default ape). The
  server gives each joiner a random variant no connected player has (any, once all eight are taken) in
  `_c_colour` before the welcome; it is not saved, a rejoin may draw another. The local body uses it too.
- **Map and teleport.** The minimap and the map screen draw every other player as a square in their coat
  colour with the name (minimap: on the rim when out of view). `/tp <player>` puts you a metre in front of
  them; the game has no operator or permission gate on commands yet (`/tp <x y z>` is open too), so neither has this.
- PROTO 6.

## Consequences
- `tools/net_proof.sh` proves bob gets alice's held item, armour, a stain, a colour other than his own and her
  map marker, and that `/tp alice` puts him beside her.
- Not done: the offhand item, the inventory portrait in the server's colour, a remembered colour per name.
