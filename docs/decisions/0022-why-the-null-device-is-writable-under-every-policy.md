# 22. Why the null device is writable under every policy

Date: 2026-11-24

## Status

Amended by the note under "Decision" on Windows.

## Context

The sandbox denies writes by default and grants them per path: the workspace
root under `WorkspaceWrite`, the scratch roots, nothing under `ReadOnly`. A
live run showed the cost of taking that literally. The model asked for
`ls -la 2>/dev/null || dir`, and the child reported

```
exit 127
sh: /dev/null: Operation not permitted
sh: dir: command not found
```

The redirect failed, so `ls` never ran, so the shell fell through to `dir`,
which does not exist. The task took three model turns to answer a request that
needed one, and the report blamed a command the model had guessed at rather
than the confinement that refused a command that could not have changed
anything.

`2>/dev/null` is not an unusual line. It is the commonest redirect a shell
command carries, written without thought by every model trained on shell
transcripts, and a sandbox that refuses it fails a large share of ordinary
work in a way whose message points at the wrong thing.

## Options considered

- Leave the policy strict and teach every command to avoid the null device:
  the confinement is right, so the command is wrong. Rejected because it is
  not the human's error and cannot be corrected from the surface; the model
  writes the command, and it will write this one again.
- Send the null device to a scratch file instead of granting the path: keeps
  one rule (writes only go somewhere), costs a file per command and makes
  `/dev/null` mean something else than it means everywhere else.
- Grant the device for writes under every policy, and say so in one place.

## Decision

The Seatbelt profile and the Landlock ruleset both allow writes to `/dev/null`,
`/dev/zero`, `/dev/stdout`, `/dev/stderr` and `/dev/fd`, under every policy
including `ReadOnly`. The null device discards what is written to it and the
descriptors are the child's own, so the grant adds no reach the policy was
protecting against: there is nothing on the other end of any of them.

The rule is written once in `crates/pearl-tools/src/sandbox.rs` (with the Linux
counterpart in `crates/pearl-tools/src/linux.rs`) and is proved by
`given_a_command_that_throws_its_errors_away_when_it_runs_then_it_still_runs`,
which runs under both policies and on every platform CI has.

## Consequences

A shell command may write to the five device paths named above whatever the
policy says. A future device that carries data (`/dev/tty` is the obvious one)
is not in that list and must not be added by analogy: writing to the terminal
would let a command paint over the surface the human is reading.

Cost: two lists of device paths to keep in step — macOS in a profile string,
Linux in a ruleset — and the Linux half is not exercised by a macOS
development machine, only by CI. The test is written for both platforms so the
gap is a delay in finding a mistake rather than a place one can hide.

## Amendment: the Windows null device

On Windows the promise is kept only for `NUL` where the machine allows it. A
measured run (`crates/pearl-tools/tests/sandbox-tests.rs`, the story gated with
the reason on it) shows an AppContainer refused `2>NUL` with `Access is denied.`
under both policies, and the grant attempted from the profile does not change
that: `\Device\Null`'s DACL belongs to the system, and a process without
`WRITE_DAC` on the device cannot add the AppContainer's ACE to it. The scratch
tree, the capture, and a denial naming what it refused are all answered on
Windows; this one device is not.

Cost: a Windows model that writes `2>NUL` reads an `Access is denied.` it did
not cause, and the notice line is the only thing that can tell it to drop the
redirect. The story returns when the machine gives the device's DACL, or when
the sandbox gives a command a null of its own that `cmd` resolves first.

