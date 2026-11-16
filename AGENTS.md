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
  directory. `docs/decisions/decisions.md` is a generated table of contents
  and `.adr-sequence.lock` is the tool's numbering; neither is edited by hand.
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
  writes that link and its target for you. A record that is superseded
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

`ert-gwt` is not on MELPA yet. It is vendored at `vendor/ert-gwt/`, put on the
load path by `test/pearl-support-tests.el`, and both go when the package lands
there.

`vendor/` holds that one file and nothing else. A change that wants a
dependency writes an ADR first.

## Keep the documents honest

`README.md`, `docs/architecture.md`, `docs/glossary.md` and `docs/roadmap.md`
describe one design, in the present tense, and each claim carries whatever it
costs. A reversal rewrites the page rather than annotating it: a document that
carries both answers is worse than one that carries the wrong one. The
`doc-links` check fails a broken link or anchor, so a page that moves takes its
inbound links with it.
