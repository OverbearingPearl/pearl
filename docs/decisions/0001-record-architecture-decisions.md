# 1. Record architecture decisions

Date: 2026-11-16

## Status

Accepted

## Context

PEARL departs from Pi in eight places, and every departure has a price. Held
in one prose page, those departures rot in a specific way: reversing one
decision forces a rewrite of the page, the rewrite erases the record that the
reversal happened, and a reader cannot tell which paragraph is current. The
page also grows without bound, so the cost of the argument stops being
visible against the size of the document.

The project already answers questions in public: which Pi modules are copied,
which choices are ours, what each one costs. What it lacked was a unit of
change for that answer.

## Options considered

- One long `docs/decisions.md`, renumbered by hand. Rejected: a reversal
  forces an edit to the one file every other document links into, and the
  edit is indistinguishable from a new decision.
- Comments in the source. Rejected: the reasoning is about the shape of the
  package, not about the file it happens to sit in, and it is invisible to
  anyone deciding whether to use PEARL.
- Architecture Decision Records in `docs/decisions/`, one file per decision,
  numbered and append-only.

## Decision

Every decision that departs from Pi, or that a future contributor could
otherwise relitigate, is recorded as an ADR under `docs/decisions/`, one file
per decision, named `NNNN-kebab-title.md` and numbered in order of writing.

Records are written with `scripts/adr new "Title"`, which runs adr-tools and
then leaves the generated files hygiene-clean. A decision is never edited
into a different decision: the record that replaces it is a new ADR, and the
old one changes only its `## Status` line and moves to
`docs/decisions/superseded/`.

The body follows the Nygard shape with one addition: a `## Options
considered` section, and a line beginning `Cost:` in `## Consequences`,
because a choice with no stated price is not a choice.

## Consequences

A reversal costs one new file plus a status line, and the old reasoning
survives next to the argument that replaced it. Every document that used to
point into `docs/decisions.md` now points at one ADR, so a page that
discusses two decisions carries two links.

A reader who wants the whole design still has to read several files. That is
the intended trade: no single page is authoritative, and none of them can be
silently overwritten by a rewrite.
