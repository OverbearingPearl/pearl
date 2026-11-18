# 30. Why undo is a view over the root and an act on one branch

Date: 2026-11-18

## Status

Accepted

Supersedes [23. Why an undo is scoped to a session and as narrow as the change](superseded/0023-why-an-undo-is-scoped-to-a-session-and-as-narrow-as-the-change.md)

## Context

ADR 23 scoped an undo to the session that performed it, with the argument that
a diff of the tree answers "what changed" and not "who changed it", so a user
reading one transcript must not be able to revert another's work.

That argument is about the act and was applied to the view. The consequence is
the one it was trying to prevent: a change that cannot be undone is not
explained, it is simply out of reach, and a user who cannot see why learns that
the feature is unreliable rather than that the change belongs to somebody else.
The information needed to explain it already exists - the change, its entry, the
session that wrote it - and hiding it costs the user the one page that would
answer "what has been happening to this directory".

What the record already provides is most of the machinery a tree view needs. A
session is a tree with `:id` and `:parent` rather than a log (ADR 3), every
change is a patch carried by the entry that made it (ADR 23, ADR 25), and
entries are never modified or removed (ADR 27). Two consoles on one root are
already supported, so several sessions editing one directory is the normal case
rather than a special one.

## Options considered

- Keep the session scope and add a command that says who wrote a file. The user
  still has to know to ask, and the answer arrives as an unordered list with no
  relationship to the work.
- One log file at the root that every session appends to. It gives a total order
  for free and invents a second writer and a second answer to "what happened",
  which ADR 27 refused once already.
- A single pointer over the whole root: "return every file to how it looked at
  this moment". It reverts work the user wanted, and nothing can undo that
  gesture, because it is the gesture.
- A view over the root and a pointer per branch, which is what this record
  chooses.

## Decision

The view is the root's. `pearl-undo-timeline` takes the sessions of a root and
straightens them into one list ordered by entry timestamp, ties broken by
session id, each node naming the session it belongs to. It is a projection over
the session files rather than a record of its own: readers do not lock (ADR 22),
so a live writer contributes a valid prefix and "live" means "as of the last
complete line".

The act belongs to one node on one branch. `pearl-undo-plan-at` builds the plan
from the head of that node's session back to the node, and refuses when a later
entry on another branch wrote a path the plan would restore, naming the session
and the entry that did. Reverting another session's work is allowed, and it
requires naming that session's node - the view is global by default, the act is
addressed.

An undo of another branch's change is appended to that branch, with `:by`
naming the session that asked for it. The patch belongs to the session that
wrote it and its inverse belongs there too; who asked is metadata, not
ownership.

The order is an interleaving and not a causal order. Timestamps tie, clocks
move, and a child's turn can end after the parent's next turn began. Vector
clocks would answer a question a reader of one directory does not have, and
would cost them the ability to read the page, so the limitation is stated
rather than solved.

The view is monotone, so a refresh inserts and never rebuilds. Entries are never
modified or removed, which is what lets point and folds be preserved by entry id
and what makes an incremental read possible: a session file only grows, so a
reader keeps the offset it read to and takes the bytes after it.

A refresh is an event and not a timer with a period. The view follows a writer
only when no loop is streaming; while one is, the mode line carries a count of
new nodes instead of the nodes. Once the user touches the buffer - opens a fold,
moves point, asks for a preview - the view stops following until it is asked to,
because a page that moves while it is being read is worse than a page that is a
minute old and says so.

A previewed plan is advisory and is re-derived when it is applied. If a
background write has changed what it would touch, the plan is refused with the
sentence that names what changed. Nothing is applied quietly and nothing is
refused silently.

## Consequences

The question a user actually has - "what has happened to this directory" - has a
page, and a refusal has a reason attached to it. A worker subagent's writes stop
being outside the undo domain, which is the premise the subagent record needs
(ADR 31). Ordering lives in one pure function, so the interleaving rule is
testable without a buffer, a file or a clock.

The cost of reading several files is now on the path of a UI gesture, so the
adapter's format became load-bearing for a view and not only for a replay, and
the poll has to be cheap: a stat per session file, and bytes after the last
offset. A session file that shrank - deleted, rotated, written by a version that
rewrote rather than appended - forces a full re-read, which is the one case the
monotone assumption has to survive.

Cost: the global view makes it possible to revert work the user did not do in
the conversation they are looking at, so the console owes two sentences before
it acts - whose change, and what else that session did after it - and an undo
across branches writes an entry into a session that no console of the current
one is showing. Point preservation depends on ids being stable in the record,
which they are now (ADR 27) and were not before it.
