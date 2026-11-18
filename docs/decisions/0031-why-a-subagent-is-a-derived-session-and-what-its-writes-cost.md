# 31. Why a subagent is a derived session and what its writes cost

Date: 2026-11-18

## Status

Accepted

Amends [9. What PEARL deliberately does not build](0009-what-pearl-deliberately-does-not-build.md)

## Context

ADR 9 refused a subagent with one reason: the rollback rule. A parent's plan
would have to expand a child's, and no implementation to copy has that. ADR 30
removed the reason, not by inventing the rule but by noticing that attribution
was never a mechanism - it is reading several session files instead of one, on
a page the user can see.

The benefit was reassessed at the same time, and the honest accounting is not
the one delegation usually gets. Folding covers the context argument: a range
that was explored and then folded costs one reading of it, and a delegate costs
the child's own reading plus its system prompt. Two research runs in this
repository measured 4.7 million input and 719 thousand output tokens for two
reports. Delegation is not a way to spend less; it is a way to spend the same
money on several things at once.

What it does buy is two things folding cannot: work that runs while other work
runs, and a context that only ever contains one question. In Emacs the first is
real because the bottleneck is a network response rather than a processor - four
loops waiting on four providers finish in about the time of the slowest.

## Options considered

- A subprocess per delegate, as Pi does. It needs a second program, a session
  file, an argument vector and an environment to keep in sync, to get something
  the loop can already do.
- A plain function call inside the loop. Cheapest, and it has no session, so
  nothing it does can be undone, attributed or read after the turn.
- A derived session: a session of its own, a read-only link to the parent for a
  human, and no inherited context.

## Decision

A delegate is a session with its own file and its own lock, and it inherits no
context from the parent. Its task is the prompt; its result is a file plus a
pointer of at most 500 characters announced back to the parent, because the
point of the arrangement is that the transcript never enters the parent. That
number is not a display limit: it is the size at which a result stays cheap to
keep or to fold away.

The roles that come first are the read-only ones - reviewer, researcher,
planner, oracle. They write nothing and ask nothing, so they add no entry to the
undo domain and touch no buffer the user is working in. A read-only delegate is
therefore a session file, a loop and a branch, and nothing else.

Worker delegation is the second step and it waits for the root timeline. When it
lands, a worker runs in the copy ADR 25 defines rather than in the parent's tree,
and its writes are nodes on the timeline with its own session as the branch, so a
parent's plan refuses across them by name instead of pretending they are not
there. The reason to allow it at all is not permission - it is that the change
became visible, attributable and revertible.

A delegate may not ask. A confirmation from a background loop takes the
minibuffer from a user who is typing in it, and a fleet of them takes it
repeatedly. A child either proceeds under its role or fails and says why.

Delivery is the loop's own vocabulary: a synchronous run answers as a tool
result, an asynchronous completion arrives as a steer, which is the one channel
that can reach a running turn. There is no second notification mechanism to
build, and the turn-end flush is the timing the loop already has.

Refused: a child that shares the parent's session, because ADR 22 gives a file
one writer and a shared file would be two contexts in one record; a child that
inherits the parent's projection, because it would then have to be folded into
the parent's record, which is the parent with extra steps; and a supervisor that
decides how many agents to start, because a model choosing a budget is a budget
nobody set.

## Consequences

A question that is wide and shallow - read these twenty files and tell me how
the module is layered - can now be asked without the parent paying for the
reading, and several such questions can be asked at once. A delegate's failure is
visible as a branch rather than as silence, and its writes are revertible by the
same gesture as everything else.

The cost is that the console grows a fleet: several sessions, several files,
several branches on one page, and the refresh policy ADR 30 defines has to
follow all of them. Roles are a promise and not a sandbox - a read-only role is
enforced by the tools it is given and by nothing else - so this record's
guarantee is about what PEARL hands a child, not about what a child could do.

Cost: delegation spends more tokens than doing the work in the parent, so the
justification has to be parallelism and attention every time, and a review of a
fleet should ask whether both were worth a session file, a lock, and a branch.
The 500-character pointer is a hard cap on what a parent learns without a second
read, which means the interesting part of a delegate's work is always one tool
call away and never in the conversation.
