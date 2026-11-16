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

## Record decisions in `docs/adr/`

A decision is written down when it departs from Pi, when it reverses an
earlier record, or when a reviewer could otherwise relitigate it. Ordinary
implementation choices do not need one.

- **Location.** `docs/adr/NNNN-kebab-title.md`. The number is in the name and
  in the front matter, and the name carries no prefix: the `pi` extension
  `@nicknisi/pi-adr` matches four digits at the start of a file name, and
  `docs/decisions` is a symlink to `docs/adr` because that extension looks for
  that path. Both exist so the index reaches an agent's prompt; neither is
  where a record is edited.
- **Tool.** `scripts/adr new "Title" --summary "One paragraph."` creates and
  renames. Never call `adrx` directly for a new record: it writes
  `ADR-0001-title.md`, which the index extension cannot see. `scripts/adr
  check`, `scripts/adr build` and `scripts/adr affected --base REF` go
  straight to adrx.
- **Front matter.** `title` (as `ADR-NNNN: Title`), `date`, `status`, and
  `summary` of at most 300 characters. `adrx check` enforces those four, and
  the summary is what the generated index shows.
- **Body.** `# Context`, `# Options considered`, `# Decision`, `#
  Consequences`. `Options considered` lists the routes not taken, one sentence
  each, and `Consequences` ends with a line beginning `Cost:`: a choice with
  no stated price is not a choice. The headings are level one because adrx's
  template says so; a level-two heading is accepted by the suite but not by
  the template.
- **Status.** One of `proposed`, `accepted`, `rejected`, `deprecated`,
  `superseded`, from `.adrx.config.json`. A superseded record changes only
  that line and moves to `docs/adr/superseded/`, which is what `scripts/adr
  supersede 4 "Title" --summary "..."` does. The move is the part adrx does
  not do, and the reason the wrapper exists.
- **One decision per record.** A record that needs "and" in its title is two
  records. Never edit a decision into a different decision.
- **Filenames are the index.** The `pi-adr` extension puts every `NNNN-*.md`
  in that directory into the system prompt as a veto list, so a filename has
  to say what it settles: `why-the-session-lives-in-the-core`, not
  `session-refactor`. `0000-*` and `superseded/` are excluded, and the
  generated `ACTIVE.md` and `index.json` are ignored by every reader — they
  are build artifacts, not records.
- **Read before you change.** An ADR that covers the area you are about to
  modify is settled. Do not contravene one without asking; to change it, write
  a new record that supersedes it.

## Write the story before the module

Every test is a Given-When-Then story in `ert-gwt` form, in `test/`, one file
per module, named `<unit>-tests.el`. The `:when` clause is exactly one user
action and each `:then` states an observable outcome. The roadmap's acceptance
criterion for a stage is the story that proves it, so a stage with no story
has no acceptance.

`ert-gwt` is not on MELPA yet. It is vendored at `vendor/ert-gwt/`, put on the
load path by `test/pearl-support-tests.el`, and both go when the package lands
there.

`vendor/` is a class of its own in the check suite, not a corner: its files
are discovered, put on the load path (the directory and each package
directory under it), and may be required by tests exactly like the package
itself. Business code may not require them, and no rule of ours applies to
them except whitespace hygiene -- a vendored file is somebody else's, under
somebody else's license and header.

`vendor/` holds the test framework and nothing else. A change that wants a
dependency writes an ADR first.

## Keep the documents honest

`README.md`, `docs/architecture.md`, `docs/glossary.md` and `docs/roadmap.md`
describe one design, in the present tense, and each claim carries whatever it
costs. A reversal rewrites the page rather than annotating it: a document that
carries both answers is worse than one that carries the wrong one. The
`doc-links` check fails a broken link or anchor, so a page that moves takes its
inbound links with it.
