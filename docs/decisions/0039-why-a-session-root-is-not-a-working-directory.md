# 39. Why a session's root is not a working directory

Date: 2026-11-20

## Status

Accepted

## Context

A console is opened on a directory and that directory stays with it. The
store writes under `<root>/sessions/` (`lisp/pearl-console.el:1037`), every
tool resolves its paths against `:root` from the loop's context
(`lisp/pearl-tools-read.el:56`, `lisp/pearl-tools-edit.el:97`), and the shell
runs with `:directory` set to the root (`lisp/pearl-tools-shell.el:327`).
`pearl-proc` uses that directory, falling back to `default-directory` only
when a caller has none. There is no tool that changes a directory and no
function that re-roots a console.

That leaves four questions a user can reasonably ask. Can a console move to
another root? What happens to its children if it does? Can the agent change
directory at all? And when a shell command's working directory differs from
the root, which one is in charge?

## Options considered

1. A `cd` tool that changes the loop's root. One session file would then be
   reachable from two roots: the lock would still hold (ADR 22 is per file),
   but the timeline, the `delegations/` directory and the snapshot base would
   each name a different root, and children started before the move would stay
   behind under the old one.
2. Re-root on `pearl-console-switch-session`. The same problem, plus a buffer
   whose identity changes under the user.
3. A per-command directory argument on the shell tool, as an alternative to
   `cd x && …`. A second way to say the same thing, and the shell already has
   one.
4. The root is fixed when the console opens; the agent has no cwd; a `cd`
   lasts one command. Chosen.

## Decision

- The root is chosen when a console opens and is part of what identifies it.
  There is no function that changes a root, and no tool that asks for one.
- The agent has no working directory, only a root. Every tool resolves a
  relative path against the root, and a shell command starts there.
- A `cd` inside a command lasts that command, because each command starts at
  the root again. It is a statement about one `sh -c`, not a state the agent
  carries.
- Another directory is another console: `M-x pearl` on it. Two consoles on one
  root are allowed, because ADR 22 forbids two writers of one file, not two
  consoles of one root.
- A delegated child runs under its parent's root — `sessions/` and
  `delegations/` are written there — so a second console cannot orphan a
  running child, and the parent's timeline shows it.
- `pearl-session-jsonl` paths are relative to the root, and the record stores
  relative names where it can (the edit tool writes `file-relative-name`
  against the root), so a repository that moves on disk is reopened rather
  than rewritten.

## Consequences

What the model is shown and what the tools can touch are the same directory,
which is the one thing that makes a snapshot's scope mean anything.

A task that spans two projects needs two consoles and a human to carry the
result between them. A child cannot leave its parent's root, which is also why
the parent can always find its file.

Cost: a long task that wants to work somewhere else has to spell the directory
in every command, paying for it in the model's tokens and in the user's
inability to say "just work in that folder now".
