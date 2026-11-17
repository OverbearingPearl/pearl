# 15. The loop knows its owner as a liveness predicate

Date: 2026-11-17

## Status

Accepted

## Context

Two settled records meet at one unnamed seam. ADR 10's third rule says a
run whose buffer was killed reports the death instead of dropping the
chain, and the loop's commentary says the three rules are implemented
there and nowhere else. The unit table in the architecture page says the
loop is core and may not know about buffers. Both are right; what was
missing is the shape of the thing the loop checks, and without a shape
the acceptance story for the rule could not be written.

## Options considered

- A `:buffer` key on `pearl-loop-new`: the core learns a buffer, and the layer rule breaks for one rule's sake.
- The rule in the console's run wrapper: the guarantee lives above the core, and a second surface can forget it — the convention ADR 3 refuses.
- A liveness predicate the loop checks before every dispatch: the core knows a function, not a buffer.

## Decision

`pearl-loop-new` takes `:guard`, a function of no arguments. The loop
calls it before every dispatch; nil means the run's owner is gone. The
chain then stops through the terminal callback, the busy flag is
released, and `:on-error` receives a `pearl-loop-dead-owner` condition.
The console passes a closure over `buffer-live-p` when it wires the
loop, and reports the condition instead of writing into a dead buffer.
The loop still knows nothing about buffers, files or provider names.

## Consequences

The mechanism is pinned by a core story that flips the guard mid-run;
the wiring is pinned by a console story that kills the buffer with the
close hook disabled, so the backstop is what fires rather than the clean
abort. The two stories together say both halves of the rule: the core
guarantees the report, and the product remembers to ask for it.

Cost: an owner that forgets the guard gets silence again. The console is
the only place that must remember, and its story is what remembers for
it; a second surface would owe the same story.
