# Template for a decision entry

An entry goes into the topic document it belongs to (`docs/decisions/<topic>.md`), under the heading that fits,
not into a new file. It takes the next free number at merge (the last line of `INDEX.md` plus one); the index
gets its line. Write it as what holds, not as a story. A branch keeps its entry unnumbered (`[draft]`) until
the merge.

```
### Short title [NNNN, YYYY-MM-DD]
What is decided, in one to five lines, with the reason in a clause. Exact numbers, names and formats.
The developer's call is marked as such and goes under "The developer's calls" of the topic.
Alternatives rejected: one line each, only when the reason is not obvious.
Replaces: NNNN's <part> (move that part to the topic's "No longer holds"; set its state in INDEX.md).
Open: what must still be decided later, if anything.
```

INDEX.md line: `| NNNN | Title | [topic.md](topic.md) - Heading | in force |`. States: `in force`,
`amended by NNNN`, `replaced by NNNN`, `history only`.
