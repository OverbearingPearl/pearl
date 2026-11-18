# 21. Why the prompt has one mechanism and not a seam beside it

Date: 2026-11-18

## Status

Accepted

## Context

ADR 20 left four sections and fed the last one from
`pearl-session-prompt-function`: a variable holding a function of no arguments,
documented as ADR 6's prompt seam. ADR 19 had said the console would be its
first user; ADR 20 then moved the console onto `pearl-session-set-prompt-sources`
because the defvar was the wrong channel for wiring. What remained was a seam
whose only caller is a test.

A seam with no named consumer is exactly what the roadmap says this package does
not carry. It would be defensible as speculation if the shape were right, and it
is not: the one scenario that would justify a function rather than a file is
text computed at render time — the last build's failure, the error under point,
the selected region, the turn's number. All of those need to know which session
they are for, and a no-argument global cannot. Two consoles would share one
addendum, which is the scope bug ADR 20 had just removed from the console.

## Options considered

- Keep the variable as a global addendum. No named consumer, wrong scope, and a
  user who rebinds it silently changes every session in the editor.
- Pass the session in, `(pearl-session-prompt-function SESSION)`. The dynamic
  scenario becomes possible, but now two mechanisms build the same list, and
  which one to reach for depends on scope, which is a decision nobody should
  have to make twice.
- Cut it. Sources remain the one way text enters the prompt, and dynamic text
  becomes a source an extension sets before the next turn.

## Decision

`pearl-session-prompt-function` goes. `pearl-session-prompt-sections` returns
the preamble followed by the sources the session holds, and that is the whole
list.

An extension that needs per-turn text subscribes to the loop's `turn-start`
(ADR 10's deferred dispatch is what makes that a between-turns moment) and calls
`pearl-session-set-prompt-sources` with an extra section. The projection reads
the sources on every request, so the next request carries it, and only that
console does. Rewriting or removing the preamble is advice on
`pearl-session-prompt-sections`, which is ADR 6's answer to every other case and
does not need a second mechanism beside it.

## Consequences

One way in, so a user or an extension never has to choose between a file, a
variable and a source. The prompt's scope is the session's scope, which is the
only scope that matches what a transcript is. ADR 6 loses one of its three seams
and keeps two: the tool list, and the console's major mode. ADR 20's fourth section
goes with the seam.

Cost: a user who wanted to add a paragraph without writing a file now writes a
file or advises; an extension that expected to rebind one function instead holds
a session and re-sets a source on turn start; and the prompt can no longer be
replaced by setting a variable, which is a capability removed on purpose,
because a variable is global and a prompt is not. Replacing the shipped text
entirely is still possible, and is now one spelled-out advice rather than a
variable that looks like a setting.
