# 34. Why search matches text and not a pattern

Date: 2026-11-27

## Status

Accepted

## Context

A session's turns were nearly all shell. One log held 278 `bash` calls
against 5 `read` calls, and inside those commands 166 `grep` invocations:
54 asking for line numbers, 20 for a count, 16 walking the tree. The
model was not being awkward. `read` took a path and nothing else, so it
had no range and no way to continue, and there was no way to find
anything except a shell.

That choice is expensive here in a way it is not elsewhere. Every shell
op captures a whole-workspace baseline before it runs (ADR 13) —
measured at 52 ms for 270 files in this repository — and a command that
only looks cannot change anything, so it pays for an undo nobody will
ask for. Finding and reading therefore have to be doors of their own,
or the cheap act keeps going through the expensive one.

`read` gained a range, which is what every other agent's read has. The
open question was what `search` matches. Of those 166 greps, 101 patterns
were plain text and 65 were regular expressions — and the expressions
were mostly anchors: `^fn`, `^#`, `^## `, `^\s*//`.

## Options considered

- **A regex engine, as the ecosystem does it.** `ripgrep` as a subprocess
  would be fastest and is what Claude Code and Gemini CLI shell out to;
  the `regex` crate would keep it in-process. Either is a new dependency,
  and a dependency is not a run's to take alone. Shelling out to `rg`
  also puts the act back behind a process the sandbox has to confine,
  which is the cost this tool exists to remove.
- **Text only, refusing anything else.** Honest, but `^fn` was a third of
  the real greps, and refusing it sends the common case back to the
  shell.
- **Text with a leading `^`, refusing the rest.** Covers the anchors that
  were actually used, needs no engine, and cannot quietly match the wrong
  thing: a pattern only a regex engine understands is refused with the
  name of the door that does it.

## Decision

`search` matches text. A leading `^` means the line starts with it, at
column zero, exactly as `grep` reads it. A dot is text, because
`queue.get` and `Cargo.toml` are what people search for far more often
than they are a wildcard. Anything else a regex engine owns — `$`, `[]`,
`()`, `|`, `*`, `+`, `?`, `{}`, `\` — is refused with "use bash grep for a
pattern", so the model is told which door to walk through rather than
being handed an empty answer it would read as "the code is not there".

Hits are `path:line:text`, walked in sorted order, skipping `.git` and
build output as a baseline does, capped at 200 matches and 32 KiB, with
the remainder counted. No match says how many files were searched,
because "nothing found" and "the tool did not work" are otherwise the
same answer.

## Consequences

Finding and reading no longer pay for a baseline, and the model is told
so in the tool descriptions: both say they are cheaper than a shell
command. The row names the act — `search ^fn`, `read src/x.rs:1200-1399`
— so the human sees which part was looked at.

The 65 expressions in that log do not all fit. `^\s*//` and `25\.` still
go through the shell, and pay for it. A regex engine would close that
gap and is the obvious next step; it is a dependency, so it is a question
for the human rather than a change a run makes on its own.

Cost: a second matching semantics in the product — text here, regex in
the shell — and a refusal the model has to read and act on. Two tools to
keep, whose descriptions are now part of the steering: if they stop
saying they are cheaper, the shell wins again and the baseline bill comes
back.