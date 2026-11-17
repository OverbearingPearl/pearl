# Roadmap

The order the design arrives in. The records in [decisions/](decisions/) say
what PEARL is and what each choice costs; this page says which parts exist and
which are still owed.

A stage is done when its acceptance runs, not when its code compiles. Nothing
here is a date.

| stage | what it delivers | acceptance | status |
| --- | --- | --- | --- |
| M0 | checks, CI, the documents and the decision records | `make lint` runs the same suite as CI | done |
| M0.5 | one tool call is one undo step | an applied edit is reverted by exactly one `C-/`, and a failed tool call leaves buffer and undo list untouched | |
| M1 | the data contract: message, tool, tool call, event, entry | an ERT case round-trips each one through its serialised form, and a fake provider produces a whole turn's events as data | |
| M2 | the loop: double loop, two queues, event emission | five ERT cases pin the ordering — steering between turns, follow-up only at the outer boundary, abort returning queued messages | |
| M3 | one provider: an SSE client with idle, total and byte limits | a message streams into a buffer as it arrives; a late reply from an abandoned request is dropped, not rendered | |
| M4 | the console: single buffer, tail rendering, batched redisplay, status line | 600 chunks arrive without the main thread doing more than 0.3 ms of work per chunk, and paging back through a 100k-line transcript stays under 50 ms per screenful | |
| M5 | three tools: read, edit, shell | an edit becomes exactly one `C-/`; a tool that fails leaves the buffer untouched | |
| M6 | the session: append-only entries and a JSONL adapter | kill Emacs, reopen, and the history is there, in the order it happened | |
| M7 | shell safety: write scope at the project root, confirmation when it cannot be established | a command that tries to write outside the project is refused or confirmed, and the status line says which regime is active | |
| M8 | review: report folding by deletion, `next-error` and `xref` wired to tool output | a failing build is navigable without leaving the console | |
| M9 | durable rollback: patches on entries, the per-turn snapshot, rollback plan, conflict refusal | a turn can be rolled back after a restart, and a file the user changed since is refused by name instead of overwritten | |

M0.5 comes before the loop on purpose. The promise in the README rests on
undo, undo is the one mechanism in this package with no upstream to copy, and
every other stage assumes it works. A change group that cannot be made into a
single step under a real major mode changes the shape of the product, not the
order of the work.

## What M0 through M9 deliberately leave out

MCP, codemode, a second provider, subagents, compaction, selection, and
multi-session management. Each of them is a surface, and none of them is on
the path to the promise in the README. Compaction is the exception in kind:
its seam is the projection function, so it can arrive without disturbing
anything above it.

## Before a tag

One thing is not automated because its cost is a container that compiles Emacs
from source: run CI once with `melpazoid` set to true before tagging a release
and before opening a MELPA pull request. It reports MELPA's own checklist and
builds the recipe, which is what a release is judged by; it says nothing `make
lint` has not already said, which is why it is not a gate.

## How to use this page

- A stage moves to `done` in the same change that adds its acceptance, and the
  acceptance is a story in `test/` before it is prose here.
- An acceptance that turns out to be wrong is edited here first, then in code.
  The list is the specification; the code follows it.
- Anything not on this page is not being built, and saying so is part of the
  design.
