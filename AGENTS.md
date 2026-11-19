# Instructions for pi

This file carries process: how work lands, where decisions are written, and
what a change owes the tests.

## A run restores a promise, and may add one

`.pi/prompts/evolve.md` (`/evolve`) gives the order of work: read the
state, choose one unit, write it red, make it green, close it, report; a
repair is a unit too, and its red is a proof rather than a story. A run
does not take the same kind of unit twice running while another kind has
evidence in hand. This file remains the process and wins where the two
disagree.

A run does not stop to ask about how. Bugs, cost and the feel of a
promise already made are its to fix, and so is a promise it wants to add:
a stage, a row, a record, a default, a public surface. What it may not do
alone is change or remove one -- an acceptance, a row, an accepted record,
a public surface, a budget moved up -- and each of those is one question,
with the diff that answers it. It may repair or tighten a gate and this
command; widening either is the user's. It may finish roadmap promises,
write the rows a promise needs, repair pages and checks, remove what the
evidence proves unreferenced, and commit; it moves a row to done only when
the story proves it. It stops for what a revert cannot make right: a
dependency, `vendor/`, `ci/`, or a push.

A run ends with `make lint` green, its unit green -- a story that passes,
or a proof that holds -- and a report of what it did, what it asked, and
what it added.

## Land large changes in small commits

A change that touches more than a handful of files is several commits, not
one: the decision records, the documents they retarget, the module skeletons,
and the tests each land on their own. Every commit leaves `make lint` green,
and its subject says what changed rather than which stage it belongs to.

A commit may leave `make test` red. Tests are written before the module they
describe, and a stage is done when its story passes, not when a commit is
pretty.

## Record decisions in `docs/decisions/`

A decision is written down when it departs from Pi, when it reverses an
earlier record, or when a competent engineer reading only the code and the
tests would choose the opposite. Ordinary implementation choices do not need
one, and neither does a choice the code already argues for: a record that
restates what the tests pin is context every later session pays for.

- **Location.** `docs/decisions/NNNN-kebab-title.md`, directly in that
  directory. Two other files live there and neither is a record. The numbering
  lock is state: one integer, the last number issued, tracked because without
  it the tool rescans only this directory and a record moved to `superseded/`
  hands its number out again. The table of contents is generated from the
  records, tracked because it is what a reader lands on, and rebuilt by the
  wrapper after every command -- the tool refreshes it only on `new`, so a
  renamed title would otherwise leave it behind. Neither is edited by hand.
- **Tool.** `scripts/adr new "Title"`. Never call adr-tools directly: the
  wrapper leaves the generated files with the final newline the check suite
  requires, rebuilds the table of contents from the records, and repairs the
  numbering lock. A record added by hand gets its number from the lock, which
  the wrapper repairs only when it runs.
- **Body.** `# N. Title`, then `Date: YYYY-MM-DD`, then `## Status`, then the
  Nygard sections. PEARL adds one section and one line:

  ```
  ## Context
  ## Options considered
  ## Decision
  ## Consequences        <- ends with a line beginning "Cost:"
  ```

  A choice with no stated price is not a choice. `## Options considered`
  lists the routes not taken, in one sentence each.
- **Status.** One of `Proposed`, `Accepted`, `Deprecated`, or `Superseded
  by` / `Amended by` followed by a link to the record responsible. A record
  that is superseded changes only that line and moves to
  `docs/decisions/superseded/`, fixing its relative links one level up.
  `scripts/adr new -s 4 "Title"` writes the status and the link; the move is
  yours, because the tool does not do it. Choose the word by what changed:
  `Amended by` is the same decision grown and the record stays where it is,
  `Superseded by` is a decision whose shape changed and the record moves. Only
  the second takes it out of the agent's prompt, so amending a shape change is
  a permanent tax on every session.
- **One decision per record.** A record that needs "and" in its title is two
  records. Never edit a decision into a different decision.
- **Write it when the decision stops moving.** A record written while the
  design is still being discovered documents a draft: two records an hour
  apart, the second reversing the first, are one decision recorded too early.
  The record belongs in the commit that makes the decision final, not in the
  one that starts the argument.
- **The directory is context.** `@nicknisi/pi-adr` reads every top-level record
  into the system prompt as a veto list, and PEARL has no compaction, so each
  one is paid for on every session until it moves to `superseded/`. Before
  writing a record, ask whether it will still be true when it is next read: a
  record describing a shape that has gone is a veto against the next correct
  change, and it reads as authoritative while doing it.
- **Filenames are the index.** The `pi` extension `@nicknisi/pi-adr` reads
  every `NNNN-*.md` in that directory into the system prompt as a veto list,
  so a filename has to say what it settles: `why-the-session-lives-in-the-core`,
  not `session-refactor`. `0000-*` and `superseded/` are excluded.
- **Read before you change.** An ADR that covers the area you are about to
  modify is settled. Do not contravene one without asking; to change it, write
  a new record that supersedes it.

The body style is Nygard's with the two additions above. Records written in
MADR style — YAML front matter, `## Decision Outcome` — are accepted by the
suite too, because the check reads shape rather than preference.

## Write the story before the module

Every test is a Given-When-Then story in `ert-gwt` form, in `test/`, one file
per module, named `<unit>-tests.el`. The `:when` clause is exactly one user
action and each `:then` states an observable outcome. The roadmap's acceptance
criterion for a stage is the story that proves it, so a stage with no story
has no acceptance. The story also has to fail without the feature: M6's
acceptance read "kill Emacs, reopen, and the history is there", which a test
that only reads the file back passes with nothing resumed in it, and the same
shape of sentence kept "compaction is absent" in the README after ADR 27 put
it in scope. An acceptance names what is true after the feature and what was
true before it, and the story is what tells the two apart.

`ert-gwt` is not on MELPA yet, so it is vendored at `vendor/ert-gwt/`
(ADR 14) and the suite puts it on the load path. The copy and its directory go
when the package lands there.

A story and a module's `;; TODO` are written separately, so they disagree
sometimes. The story is the specification and the prose is what changes: the
acceptance is what a user can observe, and a docstring that promises something
else is the thing that is wrong. Two have already disagreed -- a `plist-get`
one `car` too deep that could not have passed a correct parser, and a callback
the interface described as running either way while the story asserted it never
runs on failure -- and both were found by reading, not by the suite. Fix the
prose in the commit that finds it; if the disagreement means the acceptance
itself was wrong, edit `docs/roadmap.md` first, as the roadmap's own rule says.

When a module lands, delete its declarations from
`test/support-tests.el`. The `stub-orphans` check reports one that is still
there, so the rule is enforced rather than remembered. A declaration only defines a function that is
not already defined, so one left behind is invisible: the story passes against
the real module and nobody learns that the stub is still there.

`vendor/` is a class of its own in the check suite, not a corner: its files
are discovered, classified `vendor`, put on the load path (the directory and
each package directory under it), and may be required by tests exactly like
the package itself. Every per-file rule is off for that class -- checkdoc,
compilation, headers, naming, the blank-line rule, and whitespace hygiene
included -- because a vendored file is somebody else's, under somebody else's
license and header. The one rule that does reach it is `dependency-direction`:
a vendored module may require another vendored one and nothing of ours, and no
business module may require a vendored one.

`vendor/` holds the test framework and nothing else. A change that wants a
dependency writes an ADR first, and the record says what deleting the
dependency would take.

## Place a module in a layer

`config/ci.el` maps every module under `lisp/` to a layer, and `layer-order`
fails a `require` that points against the arrows. A new module therefore needs
a row in that table in the same commit: the check reports a module it does not
know rather than exempting it, which is deliberate -- an undeclared module is
one nobody placed.

The table is the only place the layers are declared. Do not derive them from
the file name or the directory; a second source of truth is how an architecture
page and a build end up disagreeing.

## Keep the documents honest

`README.md`, `docs/architecture.md`, `docs/glossary.md` and `docs/roadmap.md`
describe one design, in the present tense, and each claim carries whatever it
costs. A reversal rewrites the page rather than annotating it: a document that
carries both answers is worse than one that carries the wrong one. The
`doc-links` check fails a broken link or anchor, so a page that moves takes its
inbound links with it.

None of them is a work log. A roadmap row is a promise and the acceptance
that proves it; a record is a decision, the options it beat, and the price.
The session that produced them -- what was tried and abandoned, which commit,
which run -- belongs in the commit message and the report. A page that grows
by one paragraph per session is a log wearing the roadmap's name, and every
later reader pays for lines that stopped answering anything.

**Read the section before you write in it, and change the sentence that is
now wrong.** An addition that says what the page already says, or a bullet
where rewriting one line was the work, is a tail stitched onto a garment that
was finished: the page gets longer and the reader gets nothing. If nothing on
the page had to change for what is being added, it is a report.

**Update them in the same commit as the change that makes them stale**, not
afterwards. A module that lands moves its roadmap row to `done` in the commit
that makes its story pass; a mechanism whose name changes is renamed in the
architecture page in the commit that renames it; a decision that is reversed
gets its record in the commit that performs the reversal. The suite catches a
broken link; it cannot catch a page that is merely no longer true.

Three habits keep the set from drifting:

- After a `ci/` subtree pull, run `make lint` and read what is new. The check
  table is printed rather than committed (`ci/run-checks.sh --list`), so there
  is no generated file to rebuild: what a pull can cost is a rule the project
  does not satisfy yet, and `ci/MANUAL.md` says what each one wants.
- Before a release tag, re-read the four pages and the records top to bottom.
  The tag is the one moment where a stale sentence is published.
- When a record is superseded, read the one it replaces before writing it, and
  link both ways: the new record is the argument, the old one is the reason
  there was an argument to have.
