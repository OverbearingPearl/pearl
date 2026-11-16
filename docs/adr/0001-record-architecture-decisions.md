---
title: "ADR-0001: Record architecture decisions"
date: "2026-11-16"
status: accepted
summary: >-
  Records are files under docs/adr, scaffolded by adrx, named NNNN-slug.md, and superseded by a new record rather than an edit.
---

# Context

PEARL departs from Pi in eight places, and every departure has a price. Held
in one prose page, those departures rot in a specific way: reversing one
decision forces a rewrite of the page, the rewrite erases the record that the
reversal happened, and a reader cannot tell which paragraph is current. The
page also grows without bound, so the cost of the argument stops being
visible against the size of the document.

The project already answers questions in public: which Pi modules are copied,
which choices are ours, what each one costs. What it lacked was a unit of
change for that answer.

# Options considered

- One long `docs/decisions.md`, renumbered by hand. Rejected: a reversal
  forces an edit to the one file every other document links into, and the
  edit is indistinguishable from a new decision.
- Comments in the source. Rejected: the reasoning is about the shape of the
  package, not about the file it happens to sit in, and it is invisible to
  anyone deciding whether to use PEARL.
- Architecture Decision Records in `docs/decisions/`, one file per decision,
  numbered and append-only.

# Decision

Every decision that departs from Pi, or that a future contributor could
otherwise relitigate, is recorded as an ADR under `docs/adr/`, one file per
decision, named `NNNN-kebab-title.md` and numbered in order of writing.

Records are written with `scripts/adr new "Title" --summary "One paragraph."`,
which runs adrx and renames what it produced. The name carries no prefix on
purpose: the `pi` extension that puts the decision index in an agent's system
prompt matches four digits at the start of a file name, and `docs/decisions`
is a symlink to `docs/adr` for the same reason -- that extension looks for
that path and not for this one.

The front matter carries the number, the date, the status and a summary of at
most 300 characters. The body keeps the argument: `# Context`, `# Options
considered`, `# Decision`, and `# Consequences` ending with a line that begins
`Cost:`, because a choice with no stated price is not a choice.

A decision is never edited into a different decision. The record that replaces
it is a new one, and the old one changes its `status` to `superseded` and
moves to `docs/adr/superseded/`, which is what `scripts/adr supersede 4
"Title" --summary "..."` does.

# Consequences

A reversal costs one new file plus a status line, and the old reasoning
survives next to the argument that replaced it. Every document that used to
point into one decisions page now points at one ADR, so a page that discusses
two decisions carries two links.

Two names for one directory is a debt, taken deliberately: the tool wants
`docs/adr`, the extension wants `docs/decisions`, and a symlink costs less
than either a fork of the tool or a reimplementation of the extension.

A reader who wants the whole design still has to read several files. That is
the intended trade: no single page is authoritative, and none of them can be
silently overwritten by a rewrite.
