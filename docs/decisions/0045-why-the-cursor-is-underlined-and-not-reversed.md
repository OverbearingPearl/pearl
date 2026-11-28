# 45. Why the cursor is underlined and not reversed

Date: 2026-11-28

## Status

Accepted

## Context

ADR 24 kept reverse video for the cursor's row and for inline code, and ADR 44
turned the cursor's row into one of four attributes with one meaning each. A
human using the queue reported the row under the cursor as wrong: "the highlight
should not be a reversal".

The report is about what a reversed row is on the screen, not about its
legibility. Reverse draws a solid block of the terminal's other colour the whole
width of the pane, so the loudest thing on the surface is the line the hand
happens to be on rather than the work on it; the badge inside that row, drawn
dim, is inverted with everything else and reads as a hole in the block. The row
already says it is the cursor with `▶` in the gutter, which is one cell and
survives scrolling; the block says it a second time, in the terminal's
background.

## Options considered

1. **Keep reverse.** The standard terminal selection, and the one the human
   reported. A row is not a selection: nothing is being copied, and the block
   moves with every key.
2. **Weight.** The cursor's row would be bold, which is what a topic is; the
   code that drew reverse did so precisely to keep two bold rows from claiming
   to be the cursor, and that collision is only partly answered by the glyph.
3. **The glyph alone.** `▶` and nothing else. Honest, and too quiet: at the far
   left of a wide row the eye finds the arrow only after reading the row, and
   the cursor is the one thing on the surface that must be found first.
4. **An underline.** Drawn by the terminal from its own foreground, so it is
   not a colour; it follows the row instead of repainting it, and the eye can
   find the line without reading the badge.

## Decision

`style::cursor()` is an underline, and the queue's cursor row is the only row
drawn with it. `style::marked()` keeps reverse for one job, the badge of a task
that failed — the state that must not be mistaken for `Done` — and for the
picker's highlighted row, where a row really is a selection from a list.

The story that holds this is
`crates/pearl-tui/tests/layout-tests.rs::given_a_root_list_with_a_topic_when_it_is_drawn_then_only_the_cursor_row_is_underlined`:
one row is underlined, no row in the queue is reversed, and the topic row keeps
its weight without being the cursor.

## Consequences

The attribute vocabulary of ADR 44 gains a fifth named style from the same four
modifiers: underline alone is where the hand is, underline with weight is the
part of an example the human must type. They cannot appear on the same screen.
A task waiting for input already carries an underlined badge, so the one row
that is both the cursor and waiting shows the underline twice, on the badge and
under the row; that is two facts, not a collision.

The full-width underline runs to the pane's edge because a row's style is
applied to the row, not to its text; a terminal that draws underlined spaces
shows it as a rule under the row.

Cost: the cursor is quieter than a block, so a human who scans the pane for a
highlight finds a line rather than a band, and a terminal whose underline is
drawn low or not at all falls back to the glyph alone — which is why the glyph
was kept.