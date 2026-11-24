# Instructions for pi

This file carries process: what a run owes, what a story owes, and where a
decision is written. It is read before the work, so it states rules, not
history.

`README.md`, `docs/architecture.md`, `docs/glossary.md` and `docs/roadmap.md`
describe one design in the present tense and are updated in the commit that
makes them stale; a reversal rewrites the page rather than annotating it. None
of them is a work log: a row is a promise and the acceptance that proves it, a
record is a decision and its price, and the session that produced them belongs
in the commit message.

## What PEARL is for

The human's attention has no object while the agent generates. The visible,
editable task queue is that object, and the agent's report is what the human
reads while shaping the next task. Both halves are the product: a queue nobody
can read the results of is a status board, not the thing this project promised.

A run proves the thesis before it proves a crate. Every claim in the design
thread has to end at a line on a screen with a story asserting that line; a
claim that ends at a struct field is a claim nobody has built.

## A run restores a promise, and may add one

A run reads the state, chooses one unit, writes it red, makes it green, closes
it, and reports. A repair is a unit too, and its red is a proof rather than a
story. A run does not take the same kind of unit twice running while another
kind has evidence in hand.

A unit is chosen from the thesis, not from the next row of the roadmap. The
roadmap records what is owed; it does not decide what matters. A unit is deeper,
not bigger: it moves the product for the person, and it does not merely make a
data model more correct or a screen more pleasant to look at.

A run does not stop to ask about how. Bugs, cost, and the feel of a promise
already made are its to fix, and so is a promise it wants to add. What it may
not do alone is change or remove one — an acceptance, a row, an accepted
record, a public surface, a budget moved up — each of those is one question,
with the diff that answers it. It stops for what a revert cannot make right: a
dependency, `vendor/`, `ci/`, or a push.

A run ends with `make lint` green, its unit green, the binary watched with real
keys on a real screen, and a report of what it did, what it asked, and what it
added. A green suite is evidence about the suite; the screen is evidence about
the product.

## The stories are the specification

Every test is a Given-When-Then story in `rstest` form, named
`given_<state>_when_<action>_then_<outcome>`, in `crates/<unit>/tests/`.
`rstest` is the orthodox Rust fixture framework (ADR 6); GWT is a naming
convention on top of it, not a vendored crate.

The stories are the requirement document and the prose is its summary:
`README.md`, `docs/architecture.md` and `docs/roadmap.md` may only claim what a
story names. A story is written to be read as a requirement by someone who will
never open the file:

- **It starts where the person is** — a screen, a keystroke, a command line, a
  file on disk — never where the struct is. A story beginning "given a queue"
  is a unit test wearing a story's name: it can pass while no human can place
  anything.
- **The `when` is exactly one action.** Two actions are two stories, and the
  second is the one that gets forgotten.
- **It ends in what the person observes**: text on the screen, a badge that
  changed colour, a line in the log, an exit code. `Done` is a state; the
  observation is what that state puts in front of the eye.
- **It asserts the whole promise, not its first half** — not only that the
  work was done, but that the person can tell it was and read what came of it.
- **It fails without the feature.** An acceptance names what is true after the
  feature and what was true before it, and the story tells the two apart.

The surface is tested at two levels: `crates/pearl-agent/tests/` drives the
queue as a program, and `crates/pearl/tests/` drives the product — a key
pressed, a screen rendered, a line read back — because an assertion on a node
state cannot fail when the render forgets to draw it. A capability with no story
at the second level is one nobody has promised the human.

A story and a module's prose are written separately, so they disagree sometimes.
The story is the specification; the prose is what changes.

## A report is answered by a failing test first

Every report — a bug, "this is not right", a surprise at the terminal — is
answered in one order: first a test that fails for the reason the human is
describing, then the change that makes it pass. The first question is never
"what is broken" but **why no test saw it**.

The honest answer is usually that a story existed and was too weak, not that
there was none. The work is then to rewrite that story until it fails on the
current code. A new file is the rarer case, and it means a behaviour had no
owner:

- **No owner.** The defect lives in a seam between crates and each crate's
  stories are green. If a defect crossed a boundary, the story belongs at the
  top, in `crates/pearl/tests/`, not in the crate that happened to be nearest.
- **An offer instead of an outcome.** "You can type a task into the list" was
  true of a surface with no engine behind it.
- **A state instead of an observation.** The run finished, the node said `Done`,
  and nothing asserted that a person could see the answer.

## This file is amended by thinking, not by patching

A report is also a question about this file: was the rule missing, or was it
present and not followed? Answer that before adding a line.

- If the rule was missing, write the rule that would have prevented the class of
  defect, not the incident. A rule naming one bug is a patch; the next bug walks
  past it.
- If the rule was present and not followed, the fix is in the run, not here.
  Adding a second sentence saying it louder is how this file becomes unread.
- Amend by rewriting the section that is now wrong, here and on the four pages.
  Do not append a paragraph that repeats what a page already says: if nothing
  had to change for what is being added, it is a report, not an instruction.
- A rule earns its place by being true on the next read. Delete the ones that
  describe a shape that has gone; every line here is paid for on every session.

## Land large changes in small commits

A change that touches more than a handful of files is several commits, not one.
Every commit leaves `make lint` green, and its subject says what changed rather
than which stage it belongs to. A commit may leave `make test` red: tests are
written before the module they describe, and a stage is done when its story
passes, not when a commit is pretty.

## Place a module in a layer

`config/ci.toml` maps every crate to a layer under `[layers]`, and the
`layer-order` check fails a `use` that points against the arrows. A new crate
needs a row in that table in the same commit; an undeclared crate is reported,
not exempted. The table is the only place the layers are declared.

## Record decisions in `docs/decisions/`

A decision is written down when it departs from the ecosystem, when it reverses
an earlier record, or when a competent engineer reading only the code and the
tests would choose the opposite. Ordinary implementation choices need none.

- **Location.** `docs/decisions/NNNN-kebab-title.md`. The numbering lock
  (`.adr-sequence.lock`) is one integer, the last number issued, so a record
  moved to `superseded/` never hands its number out again. The table of contents
  (`decisions.md`) is generated from the records. Neither is edited by hand.
- **Tool.** `scripts/adr new "Title"`. The wrapper issues the number, writes the
  template, repairs the lock, and rebuilds the table of contents after every
  command.
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
- **Read before you change.** An ADR covering the area you are about to modify
  is settled. Do not contravene one without asking.
