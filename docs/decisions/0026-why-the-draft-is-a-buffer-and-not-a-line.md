# 26. Why the draft is a buffer and not a line

Date: 2026-11-25

## Status

Accepted

## Context

A task is a sentence the human writes while the work runs, and the surface
took that sentence on one line, with no caret: every keystroke appended, every
paste was flattened to spaces, and the queue row showed the last columns of
what was typed. Two consequences were reported: a paragraph pasted into the
surface became a task nobody could read, and a request long enough to be worth
writing pushed its own beginning off the screen. The terminal's own text
handling -- selection, wrapping, a caret that can be put back where a word is
wrong -- was not available inside a string that only appended.

## Options considered

- **Stay a line, scroll it.** Cheapest, and still cannot hold a second line: a
  pasted paragraph is flattened, and an arrow key does nothing.
- **A buffer with a caret and no editor keys.** Arrow keys and Backspace, as a
  text field in a form, and the chosen style's keys ignored inside it.
- **A buffer keyed by the style already chosen** -- vim's Normal and Insert
  modes, emacs's `C-a`/`C-e`/`C-k`/`C-y` -- plus one row of shared rules.
- **A full text area: scrolling pane, search, a status of its own.** That is an
  editor, and the queue is not a document to edit; the draft is one of the
  things on a screen that belongs to the work.

## Decision

The draft is a buffer with a caret, and the style the human chose edits it. It
takes its rows before the panes do -- one per line, at most
`clamp(height/3, 1, 8)` and never more than the window can spare -- and
scrolls with the caret after that. `⏎` places the task in every style, `Tab`
gives the keys to the queue and brings them back to the words, `Esc` puts the
draft down and a second `Esc` drops it, and opening a second draft is refused
with a notice saying where the first one is. A queue row stays one line tall
and shows the first line of a task, `…`, and where to read the rest.

## Consequences

The draft can hold what the human actually wrote, and the terminal's cursor is
a real position rather than the end of a string. Setting the draft down is one
keystroke, so reading the report while writing needs no copy of the words. A
task that holds a paragraph still reads as one row, so the queue does not grow
with the length of a request.

Cost: two keyboard implementations live inside the surface, one for the panes
and one for the draft, and the draft's is a subset of an editor's -- vim's
text objects and emacs's registers are not there, so a human who reaches for
`ciw` gets `c` and then `i` and then `w`. The draft's rules are checked by
`crates/pearl-tui/tests/draft-tests.rs`, which is also where the two editors'
coverage is pinned.
