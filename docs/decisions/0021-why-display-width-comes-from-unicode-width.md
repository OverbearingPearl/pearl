# 21. Why display width comes from unicode-width

Date: 2026-11-24

## Status

Accepted

## Context

Every row of the surface is fitted to a number of columns, and until now that
number was counted in `char`s. A `char` is not a column: a task title written in
Chinese draws two cells per character, so a row of forty characters took eighty
columns, ran past the pane, and was clipped by the terminal without a mark.
Worse, the parts of a row that are fitted together -- the bill, the child count,
the title -- were laid out against a width that did not match the glass, so the
bill fell off the end of a row that had "fitted".

The count cannot be right with arithmetic on the code point: East Asian wide
characters, combining marks, and emoji that take two cells all disagree with
`char` count, and a title is whatever the human typed.

## Options considered

Take `unicode-width` as a direct dependency, which is what ratatui already
resolves to and uses to place the same characters on the same rows.

Write the width table into this repository, where it would have to be updated
by hand and would be wrong for exactly the scripts this project's users type.

Declare Chinese titles out of scope and keep counting characters, which is what
produced the bug.

## Decision

`unicode-width` is a direct dependency of `pearl-tui`, and every fit -- the
queue row, the report line, the prompt, the help table -- measures with
`UnicodeWidthStr` rather than `chars().count()`.

## Consequences

Rows fit the glass for every script, and the mark that says "there is more" is
drawn at the column where the text actually stops. The same measurement is
already inside ratatui's own layout, so the two agree by construction.

Deleting the dependency means writing a width table into `fit.rs` and keeping
it current: the code that needs it is one module, but the table is every
Unicode version this program outlives.

Cost: one more direct dependency in `pearl-tui`; the crate is already compiled
as part of ratatui, so the build grows by nothing and the risk is that its
East Asian Width data lags a Unicode release.
