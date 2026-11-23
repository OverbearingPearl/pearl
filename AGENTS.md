# Instructions for pi

This file carries process: how work lands, where decisions are written, and
what a change owes the tests.

## A run restores a promise, and may add one

A run reads the state, chooses one unit, writes it red, makes it green, closes
it, and reports. A repair is a unit too, and its red is a proof rather than a
story. A run does not take the same kind of unit twice running while another
kind has evidence in hand.

A run does not stop to ask about how. Bugs, cost, and the feel of a promise
already made are its to fix, and so is a promise it wants to add. What it may
not do alone is change or remove one — an acceptance, a row, an accepted record,
a public surface, a budget moved up — and each of those is one question, with
the diff that answers it. It stops for what a revert cannot make right: a
dependency, `vendor/`, `ci/`, or a push.

A unit is deeper, not bigger. A run ends with `make lint` green, its unit green,
and a report of what it did, what it asked, and what it added.

## Land large changes in small commits

A change that touches more than a handful of files is several commits, not one.
Every commit leaves `make lint` green, and its subject says what changed rather
than which stage it belongs to. A commit may leave `make test` red: tests are
written before the module they describe, and a stage is done when its story
passes, not when a commit is pretty.

## Record decisions in `docs/decisions/`

A decision is written down when it departs from the ecosystem, when it reverses
an earlier record, or when a competent engineer reading only the code and the
tests would choose the opposite. Ordinary implementation choices do not need one.

- **Location.** `docs/decisions/NNNN-kebab-title.md`. The numbering lock
  (`.adr-sequence.lock`) is one integer, the last number issued, so a record
  moved to `superseded/` never hands its number out again. The table of contents
  (`decisions.md`) is generated from the records. Neither is edited by hand.
- **Tool.** `scripts/adr new "Title"`. The wrapper is self-contained: it issues
  the number, writes the template, repairs the lock, and rebuilds the table of
  contents after every command.
- **Body.** `# N. Title`, then `Date: YYYY-MM-DD`, then `## Status`, then
  `## Context`, `## Options considered`, `## Decision`, `## Consequences` ending
  with a line beginning `Cost:`. A choice with no stated price is not a choice.
- **Status.** `Proposed`, `Accepted`, `Deprecated`, or `Superseded by` /
  `Amended by` with a link. A superseded record moves to `superseded/`; an
  amended one stays. Only the move takes it out of the prompt.
- **One decision per record.** A record that needs "and" in its title is two.
- **Write it when the decision stops moving.** The record belongs in the commit
  that makes the decision final.
- **The directory is context.** Each top-level record is paid for on every
  session until it moves to `superseded/`. A record describing a shape that has
  gone is a veto against the next correct change.
- **Read before you change.** An ADR that covers the area you are about to
  modify is settled. Do not contravene one without asking.

## Write the story before the module

Every test is a Given-When-Then story in `rstest` form, named
`given_<state>_when_<action>_then_<outcome>`, in `crates/<unit>/tests/`. The
`when` is exactly one action and each `then` states an observable outcome. The
story must fail without the feature: an acceptance names what is true after the
feature and what was true before it, and the story tells the two apart.

`rstest` is the orthodox Rust fixture framework (ADR 6); the GWT shape is a
naming convention on top of it, not a vendored crate. A story and a module's
prose are written separately, so they disagree sometimes: the story is the
specification and the prose is what changes.

## Place a module in a layer

`config/ci.toml` maps every crate to a layer under `[layers]`, and the
`layer-order` check fails a `use` that points against the arrows. A new crate
needs a row in that table in the same commit; an undeclared crate is reported,
not exempted. The table is the only place the layers are declared.

## Keep the documents honest

`README.md`, `docs/architecture.md`, `docs/glossary.md`, and `docs/roadmap.md`
describe one design, in the present tense. A reversal rewrites the page rather
than annotating it. Update them in the same commit as the change that makes them
stale. None of them is a work log: a roadmap row is a promise and the acceptance
that proves it; a record is a decision, the options it beat, and the price.

Read the section before you write in it, and change the sentence that is now
wrong. If nothing on the page had to change for what is being added, it is a
report, not a document.