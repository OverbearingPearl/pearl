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

## A run proves the thesis, not the layer it enjoys

The design thread is the specification too, and it outranks the module list. A
run picks a unit from the roadmap and works until it is green, which means the
roadmap -- not the thesis -- decides what gets built, and the thesis is only
consulted when it happens to be convenient.

PEARL exists to fill one gap: the human's attention has no object while the
agent generates. The editable queue is that object. So a run owes the thesis an
answer before it owes a crate an implementation: *what does a person see, do and
read here, and would this still hold the attention it was built to hold?* A unit
that only makes the data model more correct has not moved the product, and a
unit that makes the surface more pleasant to look at has not either.

Two habits follow, and both were missing when a surface ran tasks and showed
only `✓`:

- **Trace the promise to the eye.** Every claim in the design thread -- flow,
  single-thread attention, a report someone reads -- has to end at a line of
  text on a screen, with a story that asserts that line. A claim that ends at a
  struct field is a claim nobody has built yet.
- **Watch the product before calling a unit done.** Run the binary, drive it with
  keys, read what it puts on the screen. A green suite is not evidence that a
  person can use the thing; it is evidence about the suite.

The failure this rule is named after: two design conversations established that
the queue is the flow object and that the code is "the output you occasionally
inspect" -- and then every milestone built the queue, the runner, the sandbox
and the fold, and no milestone built the inspecting. The gap was not a test.
It was a design thread that was read as background instead of as requirements.

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

## The stories are the specification

Every test is a Given-When-Then story in `rstest` form, named
`given_<state>_when_<action>_then_<outcome>`, in `crates/<unit>/tests/`. The
`when` is exactly one action and each `then` states an observable outcome. The
story must fail without the feature: an acceptance names what is true after the
feature and what was true before it, and the story tells the two apart.

`rstest` is the orthodox Rust fixture framework (ADR 6); the GWT shape is a
naming convention on top of it, not a vendored crate.

The stories are the requirement document, and the prose is its summary:
`README.md`, `docs/architecture.md` and `docs/roadmap.md` may only claim what a
story names. A story is therefore written to be read as a requirement by someone
who will never open the file, and the rules for that are not stylistic:

- **It starts where the person is** -- a screen, a keystroke, a command line, a
  file on disk -- and never where the struct is. A story that begins "given a
  queue" is a unit test wearing a story's name: it can pass while no human can
  place anything.
- **It ends in what the person observes**: text on the screen, a badge that
  changed colour, a line in the log, an exit code. "the node is `Done`" is a
  state, not an observation; the observation is what that state puts in front of
  the eye.
- **The action is one keystroke, one command, one word typed.** Two actions in a
  `when` are two stories, and the second is the one that gets forgotten.
- **It asserts the whole promise, not its first half.** Not only that the work
  was done, but that the person can tell it was done and read what came of it. A
  surface that runs a task and shows `✓` and nothing else has kept the first
  half and dropped the second; a story asserting only the first half is how it
  happened.

This is why the surface is tested at two levels on purpose: `crates/pearl-agent/
tests/` drives the queue as a program, and `crates/pearl/tests/` drives the
product -- a key pressed, a screen rendered, a line read back -- because an
assertion on `NodeState` cannot fail when the render forgets to draw it. A
capability with no story at the second level is a capability nobody has promised
the human.

## A report becomes a failing test before it becomes a fix

Every report -- a bug, "this is not right", a surprise at the terminal -- is
answered in one order: first a test that fails for the reason the human is
describing, then the change that makes it pass. The first question is never
"what is broken" but **why no test saw it**, and the honest answer is often that
a story already existed and was too weak, not that there was none. A fix that
skips that answer leaves the same gap for the next defect.

So a report usually lands on an existing story rather than in a new file. The
story was written badly -- from the implementation, or about an offer instead of
an outcome -- and the work is to rewrite it until it fails on the current code.
A new file is the rarer case, and it means a behaviour had no owner at all:

- **No owner.** The defect lives in a seam between crates, and each crate's
  stories are green. `crates/pearl/tests/` exists for the whole product: the
  surface and the runner over one queue. If a defect crossed a boundary, the
  story belongs at the top, not in the crate that happened to be nearest.
- **The acceptance described an offer, not an outcome.** "You can type a task
  into the list" was true of a surface with no engine behind it. An acceptance
  says what the user gets done, and the story asserts the done state.
- **A story asserted the mechanism, not the promise.** A test that passes when
  the feature is absent is written from the code instead of from the user, and
  it has to be rewritten rather than extended. The same defect arrives as "the
  story ends in a state, not in an observation": the run finished, the node said
  `Done`, and nothing asserted that a person could see the answer.

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