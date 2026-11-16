# 13. Keep the decision log in plain numbered files

Date: 2026-11-17

## Status

Accepted

## Context

ADR 1 fixed the shape of a record and the directory it lives in. It left the
tool open, and a richer one was tried: front-matter metadata, a validator, an
index generator, and a command that reports which records a change touches.

That tool hardcodes both its directory (`docs/adr`) and its file names
(`ADR-0001-title.md`). The `pi` extension that puts the decision index into an
agent's system prompt hardcodes its own: `docs/decisions`, and four digits with
no prefix. Neither is configurable.

## Options considered

- Keep the richer tool, and bridge the difference: a symlink for the
  directory, a rename in a wrapper, two names to explain.
- Keep the plain tool, which writes the names and the directory the extension
  reads, and take the metadata and the index as prose in the records instead.

## Decision

The plain tool. `docs/decisions`, `NNNN-kebab-title.md`, Nygard's body with the
two additions ADR 1 describes.

## Consequences

Nothing in the record-keeping path needs an adapter: the file the tool writes
is the file the extension reads, and the check suite validates the same file
the agent is shown. A contributor has one directory and one naming rule to
learn.

The richer tool's validator duplicated a check this repository already runs on
every commit, with its own idea of which sections exist, so the two had to be
kept in agreement by hand. Its index was generated with a timestamp, which
makes it a file that changes on every build; it was ignored rather than read.

Cost: no summary field to render in an index, and no way to ask which records
a change touches. The directory listing is the index, and a reviewer reads the
two or three records the change is near. If that ever stops being enough, the
feature arrives as a command over these files, not as a directory of its own.

What would change our mind: the index extension gaining a configurable
directory and file pattern. Then the constraint disappears and the choice is
about features again.

## See

- [ADR 1](0001-record-architecture-decisions.md) — where records live and what
  a record says.
- [ADR 12](0012-layer-rules-are-checked-not-documented.md) — the same habit in
  another place: a rule this repository can check itself rather than one it
  imports.
