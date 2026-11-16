# Instructions for pi

This file carries process: how work lands, where decisions are written, and
what a change owes the tests. The commit message style is deliberately not
here. It lives in `.pi/ai-git-commit.json`, field `instructions`, which the
`/git-commit` extension reads directly; a message written by hand follows the
same rules. Read that file before writing one.

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
earlier record, or when a reviewer could otherwise relitigate it. Ordinary
implementation choices do not need one.

- **Location.** `docs/decisions/NNNN-kebab-title.md`, directly in that
  directory. Two other files live there and neither is a record: the numbering
  lock is tracked state and is edited only by the wrapper's repair step, and
  the table of contents the tool writes is deleted by the wrapper and ignored,
  because it is rewritten on every `new` and never when a record is renamed.
  `scripts/adr list` is the index.
- **Tool.** `scripts/adr new "Title"`. Never call adr-tools directly: the
  wrapper is what leaves the generated files with the final newline the check
  suite requires. A record added by hand gets its number from the lock, which
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
  by` / `Amended by` followed by a link to the record responsible; adr-tools
  writes that link and its target for you, with `scripts/adr new -s 4
  "Title"`, and the file then moves to `docs/decisions/superseded/` by hand,
  because the tool does not move it. A record that is superseded
  changes only that line and moves to `docs/decisions/superseded/`, fixing
  its relative links one level up. `scripts/adr new -s N "Title"` writes the
  status and the link; move the file yourself, because the tool does not.
- **One decision per record.** A record that needs "and" in its title is two
  records. Never edit a decision into a different decision.
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
has no acceptance.

`ert-gwt` is not on MELPA yet, so it is vendored at `vendor/ert-gwt/`
(ADR 14) and the suite puts it on the load path. The copy and its directory go
when the package lands there.

When a module lands, delete its declarations from
`test/pearl-support-tests.el`. A declaration only defines a function that is
not already defined, so one left behind is invisible: the story passes against
the real module and nobody learns that the stub is still there.

`vendor/` is a class of its own in the check suite, not a corner: its files
are discovered, put on the load path (the directory and each package
directory under it), and may be required by tests exactly like the package
itself. Business code may not require them, and no rule of ours applies to
them except whitespace hygiene -- a vendored file is somebody else's, under
somebody else's license and header.

`vendor/` holds the test framework and nothing else. A change that wants a
dependency writes an ADR first, and the record says what deleting the
dependency would take.

## Keep the documents honest

`README.md`, `docs/architecture.md`, `docs/glossary.md` and `docs/roadmap.md`
describe one design, in the present tense, and each claim carries whatever it
costs. A reversal rewrites the page rather than annotating it: a document that
carries both answers is worse than one that carries the wrong one. The
`doc-links` check fails a broken link or anchor, so a page that moves takes its
inbound links with it.

**Update them in the same commit as the change that makes them stale**, not
afterwards. A module that lands moves its roadmap row to `done` in the commit
that makes its story pass; a mechanism whose name changes is renamed in the
architecture page in the commit that renames it; a decision that is reversed
gets its record in the commit that performs the reversal. The suite catches a
broken link and a stale `CHECKS.md`; it cannot catch a page that is merely no
longer true.

Three habits keep the set from drifting:

- After a `ci/` subtree pull, run `make checks-doc`: the check table is
  generated from the suite, and the freshness check fails until it is rebuilt.
- Before a release tag, re-read the four pages and the records top to bottom.
  The tag is the one moment where a stale sentence is published.
- When a record is superseded, read the one it replaces before writing it, and
  link both ways: the new record is the argument, the old one is the reason
  there was an argument to have.
