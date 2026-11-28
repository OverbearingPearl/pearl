# 39. Why the record is a log and not a session

Date: 2026-11-28

## Status

Accepted

## Context

The append-only file was `pearl-session.jsonl`, behind a `--session` flag, read
through a `SessionStore`. It holds every run in a directory: launch marks, queue
snapshots, bills, usage, folds, tool calls and results, for as long as the
directory is worked in.

"Session" elsewhere means one conversation, from a first message to a last. So
the name promised a scope the file does not have, and it promised it in the one
direction that hurts: a human who reads `--session` expects a fresh file to
start a fresh conversation, and instead gets a share of every earlier run in
that directory (ADR 23).

The pages had already settled on the right word. ADR 7, 16 and 29, the glossary
and the architecture page all say "the log", and by the end the file was the
only thing still called a session.

## Options considered

**Keep `session` and define it locally.** A definition that contradicts the
reader's is not a definition, it is an erratum, and nobody reads the erratum
before the flag.

**`ledger`.** The word is already taken twice in this codebase: the three cost
scopes ("one ledger read at three scopes") and the undo ledger ("one ledger of
takeable writes"). A third meaning would be a fourth name for the same kind of
confusion.

**`journal`.** Imports write-ahead and crash-recovery semantics: a journal is
replayed to recover a state. This file is read to rebuild a view, and a
half-written line is skipped rather than replayed.

**`transcript`.** It would be a lie of omission: the file carries queue
snapshots, bills and usage counts, not only what was said.

**`log`.** What the pages already say, and what the file is: append-only, one
writer, read from the start.

## Decision

The crate is `pearl-log`, the type `Store`, the module `log`. The flag is
`--log`, the default file `pearl-log.jsonl` with `pearl-log.lock` beside it, and the layer in `config/ci.toml` is named `log`.

The other three words are pinned so that none of them drifts back:

- **log** -- the file and everything in it;
- **entry** -- one line of it, with a `Role`;
- **run** (or **launch**) -- one opening of pearl on a directory, which is what
  a launch mark bounds;
- **conversation** -- what the model is shown, which is a projection and not
  the file.

The third cost scope is renamed with it, from "the session" to **the
directory**: the total on the status line is what the whole directory has cost,
and calling it the log's total would have said the same number twice.

## Consequences

A `pearl-session.jsonl` left by an older build is not opened, and this was
chosen rather than suffered: a resolver that guessed would have to keep two
names alive, and a half-migrated directory is worse than a clean start with the
old file beside it.

The layer table, the `--session` flag, the store, its claim file and 78 files of
prose changed in one commit, because half a rename is a second vocabulary.

Cost: a flag users must relearn, an existing file they must move by hand, and a
word (`log`) that is generic enough to need a sentence of definition on first
use.
