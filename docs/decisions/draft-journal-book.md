# draft – The journal's book (journal step 3)

- **Status:** proposed (on a branch)
- **Date:** 2026-11-17

## Context
`docs/design/journal.md`, "The book" and step 3 of its order of building: a screen that shows each ape its
journal (ADR 0047, 0150). The data, states, message and save are step 2 (ADR 0150); `do:` and `see:` are step 4.
This builds the structure; the visual pass (colours, spacing, ornament) comes after it, so the drawing is plain
and the layout lies in named rects.

## Decision
- **J opens the book and closes it**, Esc closes it (a row of the Keys page, "Journal", rebindable). It is a
  menu (`Menu::Book`) as the inventory and the globe are: the pointer is freed, nothing is paused (no screen
  pauses; the server runs on), and it does not open over the title or the death screen.
- **Left, the list** (`book::Book::rows`): the open questions first (the noticed entries, by their question,
  as the design says: "the open questions first, then pages by age"), then the known entries under a heading
  for each age, by their title, each group in `data/journal.toml`'s order. An unknown entry is not listed and
  a group with nothing shown has no heading, so nothing gives away how many entries there are. A row too long
  for the list is cut with "..". *shortcut:* the design orders the questions that touch a low need first;
  entries name no need yet, so they stand in the file's order.
- **Right, the page** (`Book::page`): a known entry its title, its text and, under "LEADS TO", a line for each
  entry it points at that is noticed or known (its title, or its question), which chooses that entry when
  clicked; one pointed at but unknown is not shown. A noticed entry its question and nothing more. With none
  chosen, a line that says what to do, or, in a new ape's book, that nothing is noticed yet. The braces of a
  text (`{temperature}`) are not filled: no entry has them until step 4's processes.
- **Changed mark:** an entry whose state the server changes after the journal the client joined with is marked
  (a dot in its row) until it is chosen; the entry open on the page is not marked (it is in view). Client-side
  only, not saved: a client that joins again starts with no marks.
- **Text** wraps to the page between words by the book's own `wrap`: a blank line between paragraphs (a blank
  line in the entry's text), a word longer than a line broken across lines; the inventory's `wrap` is unchanged.
  The list and the page each scroll by the wheel under the pointer, as far as they run over, with a thin bar
  when they do. The scroll is held to what runs over wherever rows and lines are made (`Book::scrolls`), so a
  `Journal` message that shortens the list or the page never leaves it scrolled past its end; the chosen entry
  lowered to unknown is chosen no more, and the page starts from its top when the choice changes.
- **Text size:** all of the book's text at the HUD's own size (2 screen px a font px: the pixel font stays
  whole), the page's title one step larger (3 px, as the windows' titles); a list row 28 px, a page line 22 px
  (the menus' line), the title's room 40 px. Text at 0.75 of the HUD size broke up and was not readable.
- **A menu opening lets go** (`play::let_go`, called by `set_menu`): a pick-up held to its 0.3 s and a drink
  are dropped and the server told the drink ended, since a menu swallows the button's release (J, Tab and every
  other menu).
- **Layout** (`book::layout`): a window in the middle at most 1100 x 680 UI pixels and 24 px from the edges;
  the title row; the list (42 % of the inner width) and the page (its title over its lines) side by side; the
  hint under them. Tested at 1280 x 720, 1920 x 1080 and 640 x 360 (1280 x 720 at a UI scale of 2): every part
  inside the panel, the list left of the page, the last row and line inside their rooms, the hint fitting, at
  least 6 rows, 6 lines, 24 characters a page line and 15 a list row (at 640 x 360).
- **The chat notice of step 2 stays**, without the key in it (the design asks for none).
- **Scripts (ADR 0017):** `journal` opens or closes the book, `entry:<name>` chooses an entry in it.
- **No wire or save change:** the book reads the `ToClient::Journal` messages of ADR 0150, kept in
  `Book::states`, which replaces the bool that told the first message apart.

## Measured
- `the_open_book_costs_little_a_frame` (play profile): the book with every entry known and copper's page
  chosen, drawn at 1920 x 1080: 11.5 us a frame (8.4 us before the text grew and the scroll was clamped each frame; a busy PC, 1000 frames) on the frame thread and 392 HUD rects, which the HUD's own pass
  draws (a play run records no frame times, so the frame's whole cost with the book open was not measured).
- The first shots of the book (`out/j/book-*.png`) were taken at the default 1920 x 1080 and shrunk to 1280 wide
  by the filmstrip; the shots after the review are at 1280 x 720 (a settings file), the UI scale at its default.

## Options considered
- **The list grouped by age only**, the noticed and known together (the coordinator's reading): the design
  says the open questions come first, and the design wins (principle 5).
- **Marks saved** (on the client, by world): the design does not ask for it; a mark that outlives a session is
  a small thing to add if the developer wants it.

## Consequences
- The book is ready for step 4's starter entries: more entries only lengthen the list and scroll.
- The visual pass changes `book_draw` and the constants of `book::layout`; the tests hold the structure.
- The design's "a soft sound" and "one line over the hotbar" for a new question are still the chat line of
  step 2.
