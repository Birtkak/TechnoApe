# ADR 0164: Signs, notes and trading posts

Status: accepted (2026-11-07). No new voxel materials (block entities, ADR 0007). Protocol unchanged (PROTO 2): the new
messages are two extra RPCs on the `Net` autoload (ADR 0145), so an old client simply never sends or answers them.

## Context
Fun pass: things friends leave for each other. The game already had a wall sign (furniture, ADR 0121) with one line of
text set through the `/sign` chat command and shown only in the look-at panel, and a multiplayer server (ADR 0145) that
shares voxel edits and dropped items but deliberately keeps every block entity per client.

## Decision
**Signs.** The existing wall sign grows up instead of a second sign being added: four lines of 15 characters, written in a
parchment page (`TextScreen`, opened by right click; `/sign a|b` still works), drawn on the board as a `Label3D` in a
hard-edged monospace font (`FurnitureEntity.text_label`, visible to 48 m). The painted fake scribble is gone from the
model. A post-mounted sign is not done: wall only.

**Notes.** `paper` (two birch bark) + charcoal makes a `note`: stack size 1 (a stack carries its own `text`, so no two
ever merge). Shift + right click in the air writes or reads it (12 lines of 28); a plain click on a wall pins it as
furniture kind `note` (a page with a pin; the first three lines show, the rest is read in the screen); taking it down
returns the note with its words (`FurnitureEntity.contents`). Dropping, chests and saves already carry per-stack data.

**Trading post** (`TradingPostEntity`, a work piece like the electric furnace, `work: trading_post`, solid block).
Owner = the placer (`Net.player_name` online, none offline: the one player owns everything). 26 slots in the chest
screen's container interface: 0..7 four offers as pairs (2k = what the customer gets, 2k+1 = what they pay; these are
**templates**, a click sets them from the cursor stack without moving it), 8..16 stock, 17..25 takings. Right click opens
it for the owner (Shift: the customer view to test, `TradeView`: pay left, get right, a click is one trade); everyone
else always gets the customer view; only the owner can open the stock or break the post. Tools (anything with durability)
cannot be offered or stocked, so worn goods are never swapped for new. `PlayerInventory` got one hook (`click_override`)
and a guard so no generic path (shift, double click collect, wheel, Sort) can take a template out as an item.
A trade is exact and atomic: the shop's half (`sell`: goods out of stock, payment into takings) and the buyer's
half (pay, receive) run after one check of both sides (stock, takings room, payment in hand, room in the bag); a
refusal moves nothing and says why.

**Server (online).** `PostLedger` (pure logic, `scripts/net/post_ledger.gd`) keeps signs, notes and posts server-side
(JSON next to the player saves) and is the only place a sale happens:
- every text and slot list is cleaned again (`SignText`: control, bidi and zero-width characters out, lines/columns
  capped; slots: known tradable ids only, counts within a stack) before it is stored or relayed;
- a sale is `trade(player, pos, offer, rid)`; the answer is cached by (player, rid) so a repeated request sells nothing
  twice; the goods stay a *delivery* until the buyer acks it; a join re-sends unacked deliveries; the client keeps the
  last 64 delivered ids in its player save (`applied_trades`) so a delivery that was applied but not acked is not applied
  twice;
- the buyer's client takes the payment at the click and gives it back if the server refuses; the server runs `sell` on its
  own copy (stock and takings), so two buyers of the last item cannot both get it;
- the owner's screen takes a lock (`post_open`): sales are refused with "the owner is busy" until the owner's closing
  snapshot (`post_save`, accepted only from the owner) or a disconnect; the owner's local copy is replaced by the server's
  on opening, so a stale stock can never overwrite sales;
- only the owner may remove a post; a non-owner client also refuses the dig locally.
Placement, text edits and removal go as events (`put`, `text`, `del`) and a joining client gets a snapshot; clients create
the furniture or work piece from the event. These events are not part of the batch (they need replies), so they bypass the
per-frame batching: a few per player action, never hundreds.

## Consequences
- Open limits: a bag's slots are not used for payment or receipt (only the 36 main slots); a drop of the link between the
  click and the server's answer loses the payment unless the server did not run the sale (a delivery covers the other
  case: goods owed are given at the next join); creative brush digs can still clear a post's voxels locally, the server's
  `del` check protects the stored post but that client's view is wrong until rejoin; owner screens show the generic Sort
  button, which does nothing there; the "pixel font" is a system monospace without antialiasing, not a bitmap face (no
  font file in the repo); sign lettering is not coloured or sized; the server's block-entity state is still per client for
  everything else (ADR 0145).
- Tests: `test/test_things_left.gd` (text cleaning, sign and note save/load, a trade's stacks both ways, every refusal,
  the template guards, the ledger: validation, once-per-request-id, deliveries, owner lock and ownership, persistence);
  look sheet `tools/left_things_sheet.tscn` (`game/out/left_things.png`).
