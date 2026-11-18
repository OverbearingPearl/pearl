# 19. Why the prompt is two layers, and only one of them is the user's

Date: 2026-11-18

## Status

Accepted

## Context

`pearl-session-prompt-function` says "the default says what PEARL is and how its
tools behave", and no default was ever declared. That left the one piece of
text the model reads before the user says anything to whoever implements the
projection, which means the product's behaviour at its most consequential point
was going to be an accident.

The two harnesses this package is measured against have both settled the same
way, by different routes. Pi splits the prompt into sections and fills them
from the harness: a preamble naming the harness, one line per tool, rules
derived from the tools actually selected, pointers to its own documentation,
`<project_instructions path="AGENTS.md">` read from the agent directory and
every ancestor of the working directory, and an `<available_skills>` index whose
bodies are read on demand by the read tool. Scalpel keeps a constant prompt
naming its role and its idioms, appends rule text from its own module, and
attaches user fragments from `~/.scalpel/prompt.VALUE` matched by git author,
repository or language. Neither one hands the harness contract to the user.

The common shape is a division of authorship rather than a mode. The harness
owns what is true of every use: who the agent is, which tools exist, what
"done" is allowed to look like. The repository owns what is true of this
project. Skills are a third thing again, and they are not a place to put
instructions: a skill is an index entry plus a file the model reads only when a
task matches, which is a mechanism for keeping token cost down, not a
mechanism for saying what the rules are.

## Options considered

- One user-editable prompt string. Every user becomes the maintainer of the
  sentences that keep the product coherent, and every repository has to restate
  "edits land without per-hunk approval" or the model starts asking.
- Only a project file, no shipped text. A checkout without one gets an agent
  that has never been told what its tools are, and the first thing every
  project has to write is the paragraph all projects need.
- A shipped prompt derived from the loadout, plus one project file read at the
  console, composed through the extension seam.

## Decision

`pearl-session-default-prompt` ships with the package and takes the tool
declarations the request carries. It names the tools that exist and nothing
else, so a tool the loop was not given is not offered in prose, and it states
the three things a tool schema cannot: an edit lands without per-hunk approval
because undo is the net, an edit is anchored by exact text so the smallest
change that compiles is the right one, and a write the turn's snapshot cannot
cover will be confirmed. It is a constant, not a defcustom.

`AGENTS.md` at the console's root is the project's half, read once when the
console is wired and injected as a tagged section after the shipped prompt. It
is read at the root and not above it. `pearl-session-prompt-function` replaces
both, which is ADR 6's seam doing the job it was built for: a user with a
better prompt rebinds a function rather than editing a string PEARL owns.

There is no skills index. A skill needs discovery, a file format, and a way to
load a body on demand; that is a surface, and ADR 9 says a surface is a record
of its own. When it arrives it arrives through the same function, so nothing
above the session changes.

## Consequences

A fresh checkout with no configuration gets an agent that knows its tools and
its undo, which is the same bar Pi and Scalpel hold themselves to. Project
conventions travel with the repository, where the people who know them can edit
them. And the shipped sentences improve for every user at once when they are
improved.

Cost: because the shipped prompt is a constant, a user who wants different
words loses upstream fixes and carries the whole text, which is what
`pearl-session-prompt-function` is for and also what makes it a real fork.
Because the project file is read when the console is wired, a mid-session edit
lands on the next console, not the next turn. Because a project file comes
from the checkout, it is instructions the agent did not write and did not ask
for: it is read only for a root the user named, never from a parent directory,
and the shell confirmation still gates every write it asks for. A prompt that
reads a file at a path the user chose is a smaller surface than one that walks
to `/`; it is not a trust model.
