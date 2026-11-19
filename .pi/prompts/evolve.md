---
description: Evolve PEARL one story at a time, from the documents down.
argument-hint: "[iterations]"
---

Evolve PEARL in the order its own documents prescribe: a document states
what is true, a story proves it, the code makes it true again.

Arguments: `${ARGUMENTS:-}`

The only argument is the iteration count, default 1. One iteration is one
story, from failing for the right reason to passing. Anything else stops
before a commit with one line saying that a count is the only thing read.

Do not ask for confirmation between phases except where `## Ask` says to.
Choose the smaller reversible option and say so in the report. The only
things that stop a run are listed under `## Stop`.

## The rule

A run restores a promise, and it may make one. What separates them is what
is owed the user: a restored promise needs only its story, a new one is
announced in the report, and a promise that would change or disappear is a
question first (`## Ask`).

Restoring is a document that says something the code does not do, or code
that says something no document promises, where the document is right.
Making is a new promise: a new stage, a new row, a new record, a new
default, a new public surface. Either is the run's to choose, and the
choice is a score rather than the order the roadmap happens to be written
in (`## Choosing`).

The stopping list is what no score may buy: adding a dependency, changing
`vendor/` or `ci/`, pushing or tagging.

## What counts as evolution

Not only features. Each kind of change owes a different authority, and
naming the kind is step one of choosing a story:

- **Correction** -- the code contradicts a promise it already makes: a bug,
  a message that says the wrong thing, a default that does what its page
  says it does not. A run owns it.
- **Fitness** -- the promise holds but costs too much: time, memory, a
  failure mode, a budget in `config/perf.el`. A run owns the work; moving a
  budget is the user's.
- **Experience** -- the promise holds but the meeting is poor: a message
  that does not say what to do, an order that hides the common case, a
  latency a person feels. A run owns it, and a new default is a promise it
  may make.
- **Surface** -- a capability added or removed. A run may add one; taking
  one away is an `## Ask`. Dead private code is not a surface and goes
  without an argument (`## Subtraction`).

A story says which kind it is, so the report can say what was repaired,
made cheaper, made better, added, or taken away.

## Gates and the command

The loop has machinery of its own, and it is also something to improve.

A gate is a promise made executable. The checks live in `ci/`, a shared
subtree: it is replaced by pulls and never edited here. What this
repository owns is the layer above -- `config/ci.el` for policy,
`config/perf.el` for budgets, `Makefile` and `.pre-commit-config.yaml`
for what runs when. A run may repair a gate that runs nothing (the
`text-hygiene` that passed because `git ls-files` failed), wire a check
the suite offers but the hooks skip, narrow a budget a documented promise
already implies, and carry a defect in `ci/` itself as a candidate instead
of editing it. A gate change is proved the way a gate proves anything:
break the rule once and show the check notices. Weakening a check to close
a gap is never allowed.

This command is the same kind of thing: prose the user runs. A run may
repair it against pi's actual template semantics, remove a step that no
longer has a job, and tighten a stop. It may not widen its own mandate --
a new autonomous action, a removed stop, a weaker `Never` -- because a run
that edits its own permission has approved itself. That change is a
proposal in the report, for the user to commit. A commit that moves this
file says which line of authority moved and in which direction; an edit
here is not permission for the next run until the user has taken it.

## Look outward

Every run asks what the comparable products do about this one thing, before
it chooses. The order of reference:

- pi itself: its installed source, `README.md` and `docs/` -- the thing
  PEARL is measured against (ADR 8).
- Emacs' own packages and conventions: `simple.el`, `compile.el`,
  `faces.el`, the package the slice imitates.
- any source this session can actually open -- a local checkout, or a web
  page when the session has a tool for it.

What a useful comparison yields:

- the mechanism in one sentence, and where it was read: `path:line` or a
  URL.
- what it costs there: a dependency, a default, a dial, a failure mode.
- what PEARL does differently already, and whether that difference is
  deliberate (a record) or an accident.

A comparison changes two things at most: the mechanism of the story already
chosen, or a candidate in the report. It never adds a story, a stage, or a
default on its own. A claim you could not open is marked recalled in the
report: it may shape a question, never a change. One pass per run, before
the first story; later iterations reuse it unless the area moves. When
nothing opens, say "no reference opened" and go on -- a run without a
comparison is smaller, not blocked.

## The gap is the engine

Documents and implementation disagree; that disagreement is the work. Decide
which side is the truth before touching either.

- `docs/roadmap.md` acceptance, an accepted record, and a promise in
  `README.md` are normative. The code catches up, story first.
- `README.md` details, `docs/architecture.md`, `docs/glossary.md` are
  descriptive. They change in the same commit as the code that makes them
  wrong, or in a repair commit when found later.
- Code offering a surface no document promises is either documented or
  removed; prefer the document when a user can see it.
- A promise `README.md` makes with no sentence in `docs/roadmap.md` is a
  row the run writes, with the acceptance a story can prove.

A record is the one line in that list that can stand on either side:
normative while it is current, stale once a newer normative document has
moved. `## Records` says which one it is.

Never close a gap by weakening the check or the acceptance to match the
code. An acceptance is edited when another normative document says it is
wrong -- an `## Ask`, in a commit of its own, never beside the code it
judges and never to turn a red test green.

## Records

A record forms a veto list that the next session reads before it reads the
code, so the party the veto stops is the last one who may lift it: a
supersession that changes a decision is an `## Ask`, always. What a run may
do alone is catch a record up, and whether that is what it is has a
checkable test.

**The record lags.** A newer accepted record, a roadmap acceptance, or a
promise in `README.md` already states the new shape, and the code already is
that shape. Nothing is being decided: the old record is the last document
still asserting otherwise, and `0001` names this exact failure -- a record
that stopped being true and was not superseded is a veto against the next
correct change. The run supersedes it as a repair and closes no story:

- `scripts/adr new -s <n> "Title"` writes the new record and rewrites the
  old one's `## Status`; nothing is edited by hand;
- the new `## Decision` quotes the newer sentence with `path:line` and the
  code that already carries it;
- the old file moves to `docs/decisions/superseded/`, its relative links
  fixed one level up.

The test of this shape is that the change has already happened. It changes
a document and not a promise, so `## Ask` does not apply. If the run has to
change behaviour, code or a budget to make the old record wrong, it is the
other shape.

**The shape moves.** The run wants a different decision, so the record is
the only thing that would change. That is making a promise, and the evidence
it would need is evidence no run may sign about its own constraint: an
append-only history makes the act visible afterwards, it does not make it
safe, and a run that removes the veto stopping it has approved itself.
The run puts it to the user (`## Ask`) with a package that costs one read
rather than one argument:

- the draft body -- `## Context`, `## Options considered`, `## Decision`,
  and `## Consequences` ending in its `Cost:` line;
- the old record's `## Status` line, verbatim, and the move.

The package is text: in the report, or in a scratch file under `.pi/`. It is
committed nowhere, and above all it is not written into `docs/decisions/` --
`Proposed` is a status, not a quarantine, and every top-level `NNNN-*.md`
enters the next session's prompt as settled whether the user agreed or not.
The number is issued when the user runs `scripts/adr new`, not when the run
drafts the body.

## Choosing

Selection is a score, not a queue and not the order the roadmap happens to
be written in. Before the first story, and again before each later one,
list the candidates the evidence supports and take the highest.

| term | range | what a reader checks |
| --- | --- | --- |
| Truth | 0-3 | 3 a normative promise is false; 2 a descriptive page is stale; 1 a `;; TODO` names a fragility; 0 nothing is false |
| Reach | 0-3 | 3 the first minute (a README promise, an error message, the console); 2 a normal day; 1 a stress case; 0 nobody outside the code |
| Leverage | 0-2 | 2 it unblocks another candidate; 1 it removes a recurring tax; 0 it stands alone |
| Evidence | 0-2 | 2 a gate or story is red now; 1 a measurement or a `path:line` contradiction; 0 a reading |
| Price | -3-0 | -3 it needs a yes (`## Ask`); -2 more than five files or a second session; -1 two to five; 0 one file, one commit |

`score = Truth + Reach + Leverage + Evidence + Price`.

Ties go to the higher Truth, then the lower Price, then the higher
Evidence, then the row's place in `docs/roadmap.md`.

An `Evidence` of 0 is reported and never executed: a claim that was not
opened may shape a question and nothing else. The table goes in the report,
so a score is a sentence a reader can argue with.

## Ask

Changing or removing is the user's yes, in four places: a row or an
acceptance in `docs/roadmap.md`, an accepted record whose shape moved
(`## Records`), a public surface, and a budget in `config/perf.el` moved
up. Ask once, with the smallest thing that answers it -- the diff, the
record body, the new acceptance sentence -- and go on when the yes comes. A
no ends that candidate, not the run: the next one scores.

Adding is not asking. A new stage, a new row, a new record, a new default
and a new public surface are the run's to make; the report says which kind
each one was, and a row lands with the acceptance a story proves.

## Phases

0. **Read the state.** `git status --short`, `git log --oneline -10`,
   `make lint`, `make test`. Read `AGENTS.md`, `docs/roadmap.md`, and the
   records whose filenames touch the area. A check that is green because it
   found nothing is broken, not green.
1. **Look outward, then choose one story.** One bounded comparison (see
   `## Look outward`), then the highest-scoring candidate (see
   `## Choosing`), from: a check, story or page that is untrue; a sentence
   no acceptance covers; a `;; TODO` naming a defect; a cost or failure the
   comparison exposed inside a promised behavior; a dead surface the
   mechanical signals name; a session instruction. One story per iteration;
   the candidates score again once it lands, because it moves the ground.
2. **Write the spec first.** Behavior: the acceptance sentence in
   `docs/roadmap.md` is the spec; when it is wrong, correct it there in its
   own commit first -- a new acceptance is the run's, an edit to one is an
   `## Ask`. Non-functional: name the normative home
   (`docs/architecture.md`, and a record via `scripts/adr new` when a
   settled choice moves) and the executable one (a check, a budget in
   `config/perf.el`, a row in `config/ci.el`). Descriptive pages wait for
   step 4. The comparison informs the mechanism, never the acceptance.
3. **Make the story red.** `test/<unit>-tests.el`, `ert-gwt`,
   Given-When-Then, exactly one user action in `:when`, each `:then`
   observable -- a message or an ordering is observable too. A stub in
   `test/support-tests.el` when the function does not exist. It must fail
   on its assertion, not on a void function. Commit the red story. The
   story wins over a `;; TODO` that says otherwise.
4. **Make it green** with the smallest change the story allows. Add the
   `config/ci.el` `:layers` row in the same commit a module lands; delete
   its stub. `make lint` green at every commit; no amend, rebase or push.
   Correct the descriptive pages the change made stale in this same commit.
   Do not add a second mechanism for a job one already does, and do not
   generalize a seam with one caller.
5. **Close the story.** `make test` green, or name the one story that fails
   and why. `PEARL_PERF_SCALE=2 make test` for a performance story on a
   loaded machine. Move a row to `done` only when its acceptance story
   passes, and write the row an added promise now has. A record only when
   the choice has stopped moving, at most one, and a title that needs "and"
   is two records. Re-read every page you touched and the surface the story
   changed, as a user meets it. When a stage closes, re-read its modules
   before reporting.
6. **Report** in the reply -- the reply is the journal, there is no state
   file. Keep it short (see `## Report`).

## Subtraction

Dead code is not a promise and does not need a proposal. Start from the
mechanical signals, not from taste: `contracts` (unwired signatures),
`stub-orphans`, byte-compile warnings, `grep -rn` for private names, the
`require` graph, keymaps, `add-hook`, `advice-add`, `funcall` with a
computed symbol, and any page that promises more than the code does.

Delete in one commit when the symbol is private (project prefix, no
`;;;###autoload`), `contracts` shows no caller, grep finds no hook, keymap,
advice or computed `funcall`, and `make test` is green before and after.
Unwired is not dead -- a contract with no caller may be the next promise --
so the report says what each removed symbol was. Removing a promised
surface is an `## Ask`.

## Stop

Stop before the commit, and report, when the acceptance is ambiguous in a
way that changes what is built; the only way to go on is to weaken an
acceptance or a check; the fix needs a second, unrelated failure; the check
cannot be made green; the context is low; or the next step is on the
`## Never` list. Stopping early is a complete outcome.

## Report

- what was removed or corrected, first, with the commit that can revert it.
- the slice: kind, story, and which document made it true.
- what the comparison changed: the product, the mechanism taken, and what
  was rejected.
- the score table, the candidate taken, and the one it edged out.
- before/after: `make lint`, `make test`, and the story's own result.
- the commits, by subject.
- if a gate or this command moved: what tightened, and the proof that it
  still catches the rule.
- the promises added: a row, a record, a default, a surface.
- what was asked and what came back; what is still owed, and the next
  candidate.

## Never

Push, tag, merge, release or rewrite history; add a dependency; change
`vendor/` or `ci/`; or contravene an accepted record -- superseding one is
`## Records`, not an exception to it. Change or remove a promise without
the `## Ask`, and never to turn a red test green.
