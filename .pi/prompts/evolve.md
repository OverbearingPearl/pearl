---
description: Evolve PEARL one story at a time, from the documents down.
argument-hint: "[iterations]"
---

Evolve PEARL in its own order: a document says what is true, a story proves
it, the code makes it true.

Arguments: `${ARGUMENTS:-}`

One number: iterations, default 1. Anything else, stop before a commit and
say a count is all this reads.

Work out loud, one story at a time. Do not ask permission between steps;
`## Ask` names the exceptions, `## Stop` the reasons to stop.

## The loop

0. **Read.** `git status --short`, `git log --oneline -10`, `make lint`,
   `make test`; `AGENTS.md`, `docs/roadmap.md`, and the records whose names
   touch the area. A check that is green because it found nothing is broken.
1. **Choose.** List what the evidence supports -- a red check, a red story,
   a page that lies, a `;; TODO`, a cost, a dead surface -- and say in a
   sentence which one you take and why. Several proposals are fine; the
   choice is the run's. One story per iteration, and the list is rescored
   once it lands.
2. **Story first.** `test/<unit>-tests.el`, `ert-gwt`: one user action in
   `:when`, observable `:then`s. It must fail on its assertion, and it must
   have failed before the change. Commit the red story.
3. **Green**, with the smallest change the story allows. A module lands
   with its `config/ci.el` row and without its `test/support-tests.el` stub.
   `make lint` green at every commit; pages the change made stale, in the
   same commit.
4. **Close.** `make test` green, or name the one story that fails and why.
   A roadmap row goes `done` when its acceptance story passes, not before.
   A record only once the choice has stopped moving.
5. **Report** in the reply; there is no state file.

## Where the truth is

- `docs/roadmap.md` acceptances, accepted records and `README.md` promises
  are normative: the code catches up, story first.
- `README.md` details, `docs/architecture.md` and `docs/glossary.md` are
  descriptive: fix them with the code that makes them wrong.
- Code that promises what no page does is documented or removed.
- A record is the one line that can stand on either side of that split.
- Before a story, one look at how a comparable product does this thing, and
  what that costs there. What you could not open, say you could not.

Never close a gap by weakening a check or an acceptance to match the code.

## Ask

Only when something that exists would break, in a commit of its own:

- a roadmap row or acceptance removed or reversed, or an accepted record
  contravened;
- a public surface removed, a check loosened, a budget in `config/perf.el`
  widened;
- a dependency, `vendor/`, `ci/`, a push or a tag.

Ask with the smallest thing that answers it -- the diff, the acceptance,
the record body -- and go on when the yes comes. A no ends that candidate,
not the run.

Everything else is the run's: new rows, stages, records, defaults and
surfaces, repairs, deletions, commits. The report says what.

## Records

`scripts/adr new "Title"` writes one; `-s <n>` supersedes, and the old file
moves to `docs/decisions/superseded/` with its links fixed one level up. The
body ends with a line beginning `Cost:`. One decision per record, and a
title that needs "and" is two records.

A record the code has already outgrown is a stale veto: supersede it and
say so. A decision the run wants to change is a question -- draft the body
in the report or a scratch file under `.pi/`, never in `docs/decisions/`,
where every file enters the next session's prompt as settled.

## Dead code

`contracts`, `stub-orphans`, byte-compile warnings, `grep`. Delete in one
commit when the symbol is private, has no caller, no hook, keymap, advice or
computed `funcall`, and `make test` is green either side. Unwired is not
dead; say what each removed symbol was.

## Gates

`ci/` is a shared subtree: replaced by pulls, never edited here.
`config/ci.el`, `config/perf.el`, `Makefile`, `.pre-commit-config.yaml` and
this file are ours -- repair one that runs nothing or the wrong thing, and
prove a gate catches its rule by breaking the rule once. A run may not
widen its own mandate here.

## Stop

Before the commit, and report, when the acceptance is ambiguous in a way
that changes what is built; the only way on is to weaken a check; the fix
needs a second, unrelated failure; the check cannot go green; context is
low; or the next step is on `## Never`.

## Report

Short: the story and why that one; the mechanism taken and what was
rejected; the commits by subject; `make lint` and `make test` before and
after; what was added or asked, and the next candidate.

## Never

Push, tag, merge, release, rewrite history; add a dependency; change
`vendor/` or `ci/`; change or remove anything on the `## Ask` list without
the yes; or weaken a check to get green.
