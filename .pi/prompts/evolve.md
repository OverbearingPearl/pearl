---
description: Evolve PEARL one story at a time, from the documents down.
argument-hint: "[auto|repair|propose|sweep|<stage>] [iterations]"
---

Evolve PEARL in the order its own documents prescribe: a document states
what is true, a story proves it, the code makes it true again.

Arguments: `${ARGUMENTS:-auto 1}`

The first word is a mode or a stage. If it is a bare number it is the
count and the mode is `auto`; otherwise the second word is the count.
Default `auto`, one iteration. One iteration is one story, from failing
for the right reason to passing; a stage is many iterations of the same
stage. If a stop condition fires before the count, stop and report.

Do not ask for confirmation between phases. Choose the smaller reversible
option and say so in the report. The only things that stop a run are listed
under `## Stop`.

## The rule

A run restores a promise; it does not make one.

Restoring is a document that says something the code does not do, or code
that says something no document promises, where the document is right.
Making is a new promise: a new stage, a new public surface, a new default.
The roadmap is the user's: a run moves a row to `done` when the story proves
it and writes no row of its own.

When no promised work is left, it repairs, sweeps, and reports candidates.
The next stage is the user's sentence to write.

It stops for what a revert cannot make right: reversing an accepted record,
adding a dependency, changing `vendor/` or `ci/`, removing a public surface,
pushing or tagging.

## The gap is the engine

Documents and implementation disagree; that disagreement is the work. Decide
which side is the truth before touching either.

- `docs/roadmap.md` acceptance, an accepted record, and a promise in
  `README.md` are normative. The code catches up, story first.
- `README.md` details, `docs/architecture.md`, `docs/glossary.md` are
  descriptive. They change in the same commit as the code that makes them
  wrong, or in a `repair` when found later.
- Code offering a surface no document promises is either documented or
  removed; prefer the document when a user can see it.
- A promise `README.md` makes with no sentence in `docs/roadmap.md` is a
  gap to report, not a row to write.

Never close a gap by weakening the check or the acceptance to match the
code. An acceptance is edited only when another normative document says it
is wrong, in a commit of its own -- never beside the code it judges, and
never to turn a red test green.

## Modes

- `auto` -- repair if the repository is unhealthy, else the next story of
  the earliest unfinished stage, else repair, sweep what the evidence flags,
  and report candidates. It writes no stage.
- `repair` -- truth only: a stale page, a check that finds nothing, a stub
  the suite flags. No new behavior.
- `propose` -- read the whole set and report up to three candidates: the
  acceptance sentence, the story that proves it, its cost, and what it
  displaces. Writes nothing; the user chooses one.
- `sweep` -- audit the implementation for over-design, duplication and dead
  code, and remove what the evidence proves unreferenced. See `## Sweep`.
- `<stage>` -- that stage only, for the requested count.

A candidate must be a stage one story can prove. A gap in the documents
outranks an invention, and the omissions `docs/roadmap.md` names on purpose
are not candidates unless the user asks for one.

## Phases

0. **Read the state.** `git status --short`, `git log --oneline -10`,
   `make lint`, `make test`. Read `AGENTS.md`, `docs/roadmap.md`, and the
   records whose filenames touch the area. A check that is green because it
   found nothing is broken, not green.
1. **Choose one story**, only from: a check, story or page that is untrue;
   an uncovered sentence of a roadmap acceptance; a `;; TODO` naming a
   defect; a session instruction. One story per iteration.
2. **Write the spec first.** Behavior: the acceptance sentence in
   `docs/roadmap.md` is the spec; when it is wrong, correct it there in its
   own commit first. Non-functional: name the normative home
   (`docs/architecture.md`, and a record via `scripts/adr new` when a
   settled choice moves) and the executable one (a check, a budget in
   `config/perf.el`, a row in `config/ci.el`). Descriptive pages wait for
   step 4.
3. **Make the story red.** `test/<unit>-tests.el`, `ert-gwt`,
   Given-When-Then, exactly one user action in `:when`, each `:then`
   observable. A stub in `test/support-tests.el` when the function does not
   exist. It must fail on its assertion, not on a void function. Commit the
   red story. The story wins over a `;; TODO` that says otherwise.
4. **Make it green** with the smallest change the story allows. Add the
   `config/ci.el` `:layers` row in the same commit a module lands; delete
   its stub. `make lint` green at every commit; no amend, rebase or push.
   Correct the descriptive pages the change made stale in this same commit.
   Do not add a second mechanism for a job one already does, and do not
   generalize a seam with one caller.
5. **Close the story.** `make test` green, or name the one story that fails
   and why. `PEARL_PERF_SCALE=2 make test` for a performance story on a
   loaded machine. Move the roadmap row to `done` only when its acceptance
   story passes, and write no new row. A record only when the choice has
   stopped moving, at most one, and a title that needs "and" is two
   records. Re-read every page you touched. When the stage closes, sweep
   its modules before reporting.
6. **Report** in the reply -- the reply is the journal, there is no state
   file. Keep it short (see `## Report`).

## Sweep

A sweep is maintenance, not evolution: it removes what the documents never
promised. Start from the mechanical signals, not from taste: `contracts`
(unwired signatures), `stub-orphans`, byte-compile warnings, `grep -rn` for
private names, the `require` graph, keymaps, `add-hook`, `advice-add`,
`funcall` with a computed symbol, and any page that promises more than the
code does. Unwired is not dead: a contract with no caller may be the next
stage's surface, so classify each row.

For each finding report: `path:line`, what it is (dead private code, unwired
contract, duplicate mechanism, a seam with one caller, a stale stub, a page
ahead of the code), the evidence, the minimal action, and the risk.

Delete without asking when the symbol is private (project prefix, no
`;;;###autoload`), `contracts` shows no caller, grep finds no hook, keymap,
advice or computed `funcall`, and `make test` is green before and after.
One commit per removal. Removing a public surface or a settled behavior is a
decision, not a sweep: propose a record instead. Never touch `vendor/`,
`ci/`, or the vendored framework.

## Stop

Stop before the commit, and report, when: the next step would reverse an
accepted record; it would add a dependency, change `vendor/`, or edit
`ci/`; it would remove a public surface; the acceptance is ambiguous in a
way that changes what is built; the only way to go on is to weaken an
acceptance or a check; the fix needs a second, unrelated failure; the check
cannot be made green; or the context is low. Stopping early is a complete
outcome.

## Report

- what was removed or corrected, first, with the commit that can revert it.
- the slice: stage, story, and which document made it true.
- before/after: `make lint`, `make test`, and the story's own result.
- the commits, by subject.
- the gap, if one was closed: which side was wrong.
- what is still owed, and the next story.
- the candidates, when the roadmap has no unfinished stage.
- what was refused, and why.

## Never

Push, tag, merge, release, rewrite history, write a stage into the
roadmap, add a dependency, change `vendor/`, edit `ci/`, or contravene an
accepted record.
