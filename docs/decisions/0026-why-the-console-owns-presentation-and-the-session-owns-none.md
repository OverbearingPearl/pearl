# 26. Why the console owns presentation and the session owns none

Date: 2026-11-18

## Status

Accepted

## Context

The console's contract said what it renders and how cheaply, and nothing about
how anything looks. Three questions were therefore unanswered, and each of them
is a place where an implementer improvises: what a fold is (a property of the
data, or of the view), where a face comes from (a name in the console, or a
literal in the code that draws), and what the user sees while nothing is
arriving.

The two harnesses this package is measured against answered them in opposite
directions, and both answers are coherent. Pi builds a TUI component library and
gives every event type a component, and it puts each tool's renderer next to the
tool: `grep` finds twenty-two imports of the TUI package inside pi's core. The
tool knows best how its result reads, and pays for that by making the core unable
to exist without a renderer. Scalpel goes the other way and uses Emacs's own
vocabulary: five semantic faces for the states a context entry can be in (PEARL
needs six roles, one of which is the fold itself), folds
as `invisible` text properties over content that is still in the buffer, and a
deliberate absence of animation: an ellipsis marks waiting, and parentheses say
what is being waited for.

PEARL cannot take pi's route without repealing ADR 12: the layer rule forbids a
core module requiring product code, and a renderer next to a tool is exactly
that. It can take scalpel's, and Emacs gives it two things pi's TUI does not: text
properties that survive in a buffer, and commands that can address a piece of
text by name.

## Options considered

- Renderers beside each tool, as pi does. The core stops being testable without a
  surface, and the tool list becomes a presentation registry by another name.
- No decoupling at all: let the console format strings directly from event data.
  Cheapest to write, and it makes every face, fold and status word unreviewable,
  because there is no name to review.
- Events carry data, the console renders them to sections, and the sections are
  applied as text properties.

## Decision

The core emits events with data and knows nothing about how they look. The
console renders, in two steps with a seam between them:

1. `pearl-console-render` takes an event and returns sections: a list of
   `(:text TEXT :role ROLE :fold FOLD)`. Pure, no buffer, so a story folds an
   event and asserts on the result.
2. `pearl-console-insert-event` applies the sections as text properties: a face
   chosen from the role through one table, a fold id when the section belongs to
   one, a help hint when the fold can be toggled.

Faces are a vocabulary, not colours. One defgroup names a face per role - the
user's words, the assistant's, tool output, the status line, an error, a fold
marker - and a user changes a face by customising it, not by patching a formatter.
The role is what the code says; the face is what the theme decides.

A fold is view state and never session state. Its identity comes from the event
(the message or tool call it belongs to), its state lives in the buffer, and the
session is unchanged by folding. That is what makes folding reversible and what
keeps the record free of facts about how it was displayed. A fold is also always
addressable: a key and a hint, never an implicit collapse nobody can undo.

Waiting is derived from events, not from a timer. `waiting`, `streaming`, `tool`
and `confirm` are states with different evidence - nothing yet, deltas arriving,
a command running, a question asked - and PEARL has an event for each. A timer
can only say that time has passed, which is the one thing the user already knows.

## Consequences

Every visible name is reviewable: a role, a face, a state, a fold id. A story
asserts on sections and on text properties instead of on a rendered screen, which
is a cheaper test than a component tree and a real one. A second surface can
render the same events to a different medium without the core learning about it,
and the extensions seam (ADR 6) still has something to hook.

Cost: the console owns two things that are easy to get subtly wrong and hard to
notice - property boundaries and the order of applied properties - and a section
that forgets `rear-nonsticky` inherits the previous one's face into the user's
typing. Pi's approach cannot have that bug because a component owns its whole
region; ours can, so the contract states it. And a rendering pipeline that is a
pure function tempts an implementer to render the same event twice; the story for
folds exists partly to catch that.
