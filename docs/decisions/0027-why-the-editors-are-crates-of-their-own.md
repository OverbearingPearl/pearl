# 27. Why the editors are crates of their own

Date: 2026-11-25

## Status

Accepted

## Context

The draft inside PEARL's prompt is edited by emacs's keys or vim's, chosen by
a setting. Both were written inside `crates/pearl-tui/src/app.rs`, where the
keys arrived as `crossterm::event::KeyEvent` and the words lived in a
`pearl_tui::draft::Draft`: the editor could not be handed to a test without a
terminal, could not be reused by anyone, and could not be read as an editor at
all -- `dd` was an arm in a match a thousand lines into a surface.

Three questions were open. Whether a crate already does this (emacs and vim
keymaps over a text buffer, no terminal). Whether the editors should be
libraries at all, or stay a private detail of one prompt. And if libraries,
what they speak: the terminal library's key type, or their own.

## Options considered

- **Reuse.** `edtui` is a vim editor welded to a ratatui widget and its own
  event loop; `reedline` has emacs and vi keymaps but owns the terminal and
  thinks in lines, not paragraphs; `tui-textarea` and `ratatui-textarea` have
  no modes; the rest (`vim-navigation`, `vim-line`, `hjkl`) are too new or are
  whole applications. None is a state machine a host drives.
- **Keep them inside the surface.** Cheapest now, and the fourth editor would
  also be written there.
- **Three crates: `pearl-text`, `pearl-emacs`, `pearl-vim`.** The buffer and
  the vocabulary in one, each editor in its own, a `Key` type of their own,
  and a `KeyMap` the host rewrites.
- **Speak `crossterm`'s types.** Saves the host a conversion; makes every
  editor's public API a promise to upgrade when `crossterm` does.

## Decision

Three crates, in the core layer, published together but independent:

- `pearl-text` holds `Buffer` (characters, caret, mark, undo), `Key`, `Code`,
  `Command`, `Outcome` and `KeyMap`, plus the two tables that need no state:
  `plain()` and the shared `execute()`.
- `pearl-emacs` and `pearl-vim` hold only the state a buffer does not:
  emacs's one kill, vim's mode and register. Their `map()` functions are
  defaults, not laws.
- The editors speak their own `Key`, and the host writes one conversion
  (`pearl_tui::app::as_key`).

`Command` names every key an editor answers, including `Submit`, `Leave` and
`Cancel`, which only the host can carry out and which the editor returns as
`Outcome::Host(..)`. PEARL builds its table from the style's default and binds
`⏎` to `Submit` and `⇧⏎`/`C-j` to `Break`: the editor does not read the key a
form is handed in with as a newline, because the table says it does not.

## Consequences

An editor is testable as a library: the stories in `crates/pearl-emacs/tests/`
press keys and read a buffer, with no terminal, no surface and no clock. A new
editor is a crate and a table, not a match arm. The surface keeps only what is
its own: the cap, the rows, the caret's cells, and the two verbs it owes the
form.

The cost is a boundary that has to be maintained in two directions. A key that
`crossterm` reports differently -- `C-j` arriving as a line feed, `Esc` versus
`Alt-x` -- is now the host's problem, and a behaviour the surface needs (vim
Normal mode refusing to type) has to be expressible as an editor's mode rather
than an early return in a match.

Cost: three crates, a key type that is not the terminal's, and a rebinding
table per style that a reader must consult before they can say what `⏎` does.