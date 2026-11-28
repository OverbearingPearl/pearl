# 38. Why the runner is not an agent

Date: 2026-11-28

## Status

Accepted

## Context

The crate that drains the queue was `pearl-agent`, its type `Agent<P>`, its
error `AgentError`. The word was wrong in the one way this design cares about:
an agent, everywhere else, is a thing that decides its own next step, and
PEARL's thesis is that the human owns the next step and the agent takes what
was placed. A crate named `pearl-agent` with a struct named `Agent` put that
claim in every `use` line, in the layer table and in the diagram, while the
design page said the opposite.

The repository had already half-said it: ADR 17, the glossary and
`spawn_runner` all called the thing a runner, and the crate was the only place
that had not caught up. So this was not a new word, it was the code catching up
with the document.

The word also had a second, correct job. "The agent generates", "the agent's
hands", a `subagent` in a fan-out -- there the word names the model, and it is
true there. One word was carrying the loop and the model, which is the same
defect as two words for one thing, read backwards.

## Options considered

**Keep `pearl-agent` and document the difference.** Cheapest, and it leaves
every `use pearl_agent::Agent` asserting the autonomy the README rejects. A
disagreement that lives only in prose loses to the code every time someone
reads the code.

**`pearl-harness`, `Harness<P>`.** Imports the word this design was written to
avoid: a harness wraps a model and decides how it is driven. The queue is what
decides.

**`pearl-loop`, `Loop<P>`.** Names the mechanism rather than the thing, and
`Loop<P>` reads like an iterator in every call site. It also leaves the model
unnamed, so `agent` would keep drifting back onto the loop.

**`pearl-runner`, `Runner<P>`.** The word the glossary, ADR 17 and
`spawn_runner` already use: a background task that runs whatever is placed and
waits when nothing is.

## Decision

The crate is `pearl-runner`, the type `Runner<P>`, the error `RunnerError`, the
module `runner.rs`. The layer table calls it `product`, as before.

`agent` names the model, and only the model: the thing that generates, the
thing a tool description talks to, the thing `subagent` calls a child of.
One line teaches the split, and it is the line a newcomer needs: **the agent is
the model; the runner is the loop.**

## Consequences

Every `use pearl_agent` outside this workspace breaks, and there are none the
author knows of. `git log --follow` walks the move, so the history of the loop
is not lost with the name.

Prose that said "the agent runs" now says "the runner runs", and prose that
meant the model keeps the word. The distinction is now visible on the screen
too: the status line's fold notice says the runner folded, and the report says
what the agent said.

Cost: a workspace-wide rename with no behaviour change to show for it, and a
crate name a reader has to learn before the design page makes sense.
