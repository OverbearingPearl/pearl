# 38. Why the model is chosen per request and read from the branch

Date: 2026-11-20

## Status

Accepted

## Context

A console picks its model with `C-c C-m` (`pearl-console-set-model`); the
choice lives in `pearl-console--model`, a buffer-local, and
`pearl-console-model` falls back to `pearl-console-model-default` and asks
once per console when both are empty. Nothing writes the answer down.

The record already has a place for it. The contract says an entry's kind is
"one of `message', `tool', `model', `compaction', `undo'"
(`lisp/pearl-contract.el:78`), and no code has ever written a `model` entry.

The thinking level is in the same position, one step further along.
`pearl-contract-request (MESSAGES &key TOOLS MODEL THINKING)` accepts it, and
`pearl-provider-sse-request-body` builds `:model`, `:messages`, `:stream` and
`:tools` — never a thinking field. `pearl-loop--turn` does not pass one
either (`lisp/pearl-loop.el:386`). So a caller may specify a thinking level
today and it is silently dropped at the only place that could have sent it.

A delegated child has the mirror-image defect. `pearl-delegate-start` builds
its loop with the parent's provider and root and no `:model`
(`lisp/pearl-delegate.el:158`), and that provider is the console's wrapper,
which overwrites every request's model with `(pearl-console-model console t)`
(`lisp/pearl-app.el:205-221`). A child therefore cannot run on a different
model from its parent, and it follows the parent when the parent changes
between two of the child's rounds.

## Options considered

1. Keep the buffer-local choice alone. A record that cannot say which model
   produced which answer, and a choice that dies with the buffer.
2. Keep the choice in a per-console settings file. A second source of truth
   beside the record, and one that still cannot answer "which model made this
   line".
3. Append the entry when the key is pressed. An entry for a change the user
   then abandoned, and one that sits before the turn that would have used it.
4. Append the entry when a request goes out, and read the branch to learn the
   choice that applies. Chosen.
5. Let a child consult its parent's choice on every request. The child's
   behaviour changes under it whenever the parent changes.

## Decision

The model and the thinking level are fields of a request, and the branch
records the ones each answer used.

- `pearl-session-model (SESSION)` answers what the next request would use:
  the last `model` entry on the branch, or nil when there is none.
- When a request is built and its model or thinking level differs from that
  answer, the loop appends one `model` entry — data `(:model M :thinking T)`,
  no patch, because a choice changes nothing outside the session — before the
  request goes out. One entry per change, not per turn.
- The console keeps only the next choice. What the status line shows is
  `pearl-session-model`, so the buffer cannot diverge from the record without
  saying so.
- Moving the head — checkout, rollback, resume — re-reads the branch. The
  console shows the choice of the point the head now names, which is the one
  that history used.
- A delegated child copies its parent's current model and thinking at
  creation and writes its own entry. The delegate tool's schema gains
  optional `:model` and `:thinking`, so a child can be cheaper or stronger
  than its parent; changing the parent afterwards does not touch a child that
  is already running.
- Thinking travels with the model in one entry, as two fields, because to a
  user it is one choice.

## Consequences

A resumed session can say what made its text, and the same point of a branch
always answers the same. A child's cost is not its parent's, so the audit page
can compare two children of the same turn.

Undo has to tolerate a `model` entry: it changes nothing outside the session,
so it carries no patch and a rollback walks past it. A page listing turns has
to decide that a model row is a title and not a turn.

The wire needs the field, so the adapter gains one place that names a vendor's
spelling of the level, and M15 in the roadmap does not close until a story
reads the sent body.

Cost: one entry per change means a session that tries four models carries four
nodes on its branch, and every reader of the branch — projection, timeline,
undo, audit — has to keep ignoring them correctly.
