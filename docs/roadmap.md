# Roadmap

The order the design arrives in. The records in [decisions/](decisions/) say
what PEARL is and what each choice costs; this page says which parts exist and
which are still owed.

A stage is done when its acceptance runs, not when its code compiles. Nothing
here is a date.

| stage | what it delivers | acceptance | status |
| --- | --- | --- | --- |
| M0 | checks, CI, the documents and the decision records | `make lint` runs the same suite as CI | done |
| M0.5 | one tool call is one undo step | an applied edit is reverted by exactly one `C-/`, and a failed tool call leaves buffer and undo list untouched | done |
| M1 | the data contract: message, tool, tool call, event, entry | an ERT case round-trips each one through its serialised form, and a fake provider produces a whole turn's events as data | done |
| M2 | the loop: double loop, two queues, event emission | five ERT cases pin the ordering — steering between turns, follow-up only at the outer boundary, abort returning queued messages | done |
| M3 | one provider: an SSE client over TLS, with idle, total and byte limits and a retry that stops at the first byte | a message streams into a buffer as it arrives; a late reply from an abandoned request is dropped, not rendered | done (against a local stub server; the endpoint run in [releasing.md](releasing.md) added the chunked-transfer and TLS fixes it could not have found) |
| M4 | the console: single buffer, tail rendering, batched redisplay, status line | 600 chunks arrive without the main thread doing more than 0.3 ms of work per chunk, and paging back through a 100k-line transcript stays under 50 ms per screenful | done |
| M5 | three tools: read, edit, shell | an edit becomes exactly one `C-/`; a tool that fails leaves the buffer untouched | done |
| M6 | the session: append-only entries, a JSONL adapter, and the restore that makes a file a session again | kill Emacs, reopen, and the console is at the entry it stopped on and can continue from it — reading the file back is not the test, since that passes with no restore at all | done |
| M7 | shell safety: write scope proved where the snapshot covers it, confirmation elsewhere | a command that tries to write outside the project is refused or confirmed, and the status line says which regime is active | done |
| M8 | review: report folding by deletion, `next-error` and `xref` wired to tool output | a failing build's locations reach `next-error` and `xref` from the console, and jumping to one lands in the file, because that is what the Emacs keys do | done |
| M9 | durable rollback: patches on entries, the per-turn snapshot, rollback plan, conflict refusal | a turn can be rolled back after a restart, and a file the user changed since is refused by name instead of overwritten | done |
| M10 | the timeline: undo over the root, one pointer per branch, a page that follows a live writer without rebuilding | with two consoles writing in one root, a node from the other session is undone by naming it, the page says what else that session did after it, and a reader keeps its point and its open folds when a third loop appends | done |
| M11 | read-only delegation: a delegate is a session of its own, its result is a pointer, a completion arrives as a steer | a question that is wide and shallow comes back as a report file and a pointer of at most 500 characters, the child asked nothing and wrote nothing, and the parent's conversation never contains the child's reading | done, for the four read-only roles |
| M12 | the context page: what a request is built from, drawn where the user can read it | in a console, `C-c C-p` shows the prompt above the conversation in the order the provider receives it, a compaction summary where the turns it covers stood, and an estimate on every section | done |
| M13 | an audit record: what a request cost, written where a restart cannot lose it | a console sends a line to a stub that reports its count, and the session file gains one `usage` entry per request carrying the prompt and completion counts, the time to the first delta, the total, and whether the count came from the provider or from the estimator; a stub that reports nothing yields an entry marked estimated; and neither entry reaches the model in the next request | planned |
| M14 | the audit pages: a count on every turn, three totals per session | a finished turn ends with its own count and time on one line, `C-c C-a` opens a page showing this task, this session and every turn, the session's totals are the same after closing and reopening the console, and no figure on the page is printed without saying whether it was reported or estimated | planned |
| M15 | the model and the thinking level as a fact of the request: one `model` entry where a request first uses a different one, and a status line that reads the branch | a console sends a turn on the default model, switches with `C-c C-m`, and sends another: the branch gains exactly one `model` entry between them, the body the provider receives carries the chosen model and thinking level, `C-c C-t` back to the first turn makes the next request use the first model again, and a session reopened after a restart shows the model its last answer used | planned |
| M16 | a delegate on a model of its own | a delegate asks for a cheaper model and its session file records it while the parent's console keeps its own; changing the parent's model while the child runs leaves the child's file unchanged | planned |

M0.5 comes before the loop on purpose. The promise in the README rests on
undo, undo is the one mechanism in this package with no upstream to copy, and
every other stage assumes it works. A change group that cannot be made into a
single step under a real major mode changes the shape of the product, not the
order of the work.

## What the audit numbers have to say

M13 and M14 are one requirement seen twice: a turn's price is a fact about a
request and a session's price is the sum of those facts, so the count has to be
in the record before any page can show it. Three dimensions, each derived
rather than stored a second time:

- A **turn** is one request and its answer: prompt tokens in, completion tokens
  out, the time to the first delta, the whole time. A usage entry stands on the
  branch beside the message it paid for.
- A **task** is what one instruction produced: the user entry, the turns after
  it and before the next user entry, and the tool time between them. It is a
  span of the branch, not a record of its own.
- A **session** is its whole tree since the first entry, branches the head has
  left included: turns, tasks, model time, tool time, wall time, compactions,
  rollbacks.

Every figure says which of two things it is. A provider that reports a count
is giving a fact; a provider that reports nothing is estimated with the same
arithmetic compaction budgets in (`pearl-compaction-tokens`), and the page
prints the mark that says so. One typeface for both would be a lie told with
layout, and that difference is exactly what a reader needs to decide whether
the number is worth anything.

Three clocks add up: model time plus tool time plus what is left over. The
left-over is the person, and naming it is the point — "why did this take an
hour" has three answers and only one of them is a slow model.

Where they are drawn: a turn's count ends the turn in the console, one line,
folding with it; the task and the session are a page of their own (`C-c C-a`),
because a mode line holds one word and a session's arithmetic is not a word.
None of it is a budget — a number that stopped a run would be a setting, and a
setting is the user's.

## Which model a request uses

M15 and M16 exist because the choice is a property of a request and not of a
browser tab. The console holds the next choice only; the branch holds the ones
that were used, read back as the last `model` entry before the head
([ADR 38](decisions/0038-why-the-model-is-chosen-per-request-and-read-from-the-branch.md)).
Four consequences the stages have to keep:

- Switching is free until a request goes out. Pressing `C-c C-m` and changing
your mind costs no entry, and an entry always names a choice some answer used.
- Moving the head moves the model with it, because the branch is what is read.
`C-c C-t` to an older turn and the next request uses that turn's model; that is
what makes an old answer reproducible.
- A child copies the parent's choice when it starts and writes its own entry.
It does not consult the parent afterwards, so a child's behaviour cannot change
under it and its cost is its own.
- The thinking level travels in the same entry, because to a user it is one
choice. The adapter owns the one place that names a vendor's spelling of it.


## What the roadmap deliberately leaves out

MCP, codemode, a second provider, and selection. Each of them is a surface, and
none of them is on the path to the promise in the README.

Multi-session management is half out: the timeline reads every session of a
root, and the console still opens one. Several consoles on one root is the
normal case and works; a command that switches between them, a directory view,
and searching across sessions are not built.

Read-only delegation is M11 and is built: `lisp/pearl-delegate.el` starts a
child session with its own file and lock, writes the child's answer to
`delegations/<id>.md`, and hands the parent a pointer of at most five hundred
characters. A delegate that writes is still refused by name, because a parent's
plan cannot yet expand another session's plan and name the conflict, and until
it can a worker runs in the copy of
[ADR 25](decisions/0025-why-the-boundary-is-a-copy-and-not-a-cage.md) rather than
in the parent's tree. A delegate never asks: a confirmation from a background
loop takes the minibuffer from a user who is typing in it
([ADR 31](decisions/0031-why-a-subagent-is-a-derived-session-and-what-its-writes-cost.md)).

Compaction is not in that list any more, which is the exception in kind: it is a
`compaction` entry the projection obeys, with expansion as a read
([ADR 27](decisions/0027-why-compaction-is-a-projection-and-not-a-rewrite.md)),
so it can arrive without disturbing anything above it. The command and its entry
exist; the trigger is the user's, because nobody has measured a threshold this
package could defend, and a silent rewrite of what a model reads is the one
thing [ADR 27](decisions/0027-why-compaction-is-a-projection-and-not-a-rewrite.md)
refuses.

Reopening a session is not on that list either, and it is not multi-session
management: one file is opened by one console, moving within it is moving the
head, and two writers are still refused by name
([ADR 22](decisions/0022-why-a-session-file-has-one-writer.md),
[ADR 29](decisions/0029-why-resuming-is-moving-the-head-and-reporting-the-divergence.md)).
A picker over session files, search across them and forking one into another
project are the parts that stay out.

A console's root is fixed when it opens and the agent has no working directory
([ADR 39](decisions/0039-why-a-session-root-is-not-a-working-directory.md)):
another directory is another console, and a `cd` lasts one command. Re-rooting
an open console, and a tool that changes where the next command runs, stay out
with the pickers above.

The context page prices a section the way compaction spends its budget: with an
estimate. No adapter parses the count a provider reports, so there is no true
number to print, and a page that invented one would be trusted as a bill. The
estimate is honest about being one, which is why it is on the page at all.

Subagents were refused until [ADR 30](decisions/0030-why-undo-is-a-view-over-the-root-and-an-act-on-one-branch.md)
turned the reason into a page: the rule they need was a rollback that expands
another session's plan, and expanding it is reading one more file
([ADR 31](decisions/0031-why-a-subagent-is-a-derived-session-and-what-its-writes-cost.md)).

## Before a tag

The steps are in [releasing.md](releasing.md), and the recipe the package is
published with is there too. One step is not automated because
its cost is a container that compiles Emacs from source: run melpazoid before
the tag and before opening the MELPA pull request. It builds the recipe and
runs MELPA's own checklist on it, which is what a release is judged by. It
builds MELPA's default file spec rather than ours, so the recipe stays
something a person reads, which is one more reason it is not a gate.

## How to use this page

- A stage moves to `done` in the same change that adds its acceptance, and the
  acceptance is a story in `test/` before it is prose here.
- An acceptance that turns out to be wrong is edited here first, then in code.
  The list is the specification; the code follows it.
- Anything not on this page is not being built, and saying so is part of the
  design.
