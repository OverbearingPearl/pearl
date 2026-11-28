# 18. Why the sandbox bounds writes and not reads

Date: 2026-11-24

## Status

Accepted

## Context

The confinement was a write boundary that had never been stated as one. Measured
on macOS: a write outside the workspace fails, `rm` outside fails, outbound
network fails, and `head /etc/hosts`, `ls $HOME` and `ls /` all succeed. Linux
matched it (`PathBeneath("/", from_read)`). Windows did not: an AppContainer
touches nothing ungranted, so the same `Policy::WorkspaceWrite` meant
read-nothing-outside there. One policy meant two different things depending on
the machine.

Two further facts forced the question. An agent that cannot write a scratch file
cannot use the shell for more than one-liners: a script, a log, a compiler's
intermediate all need somewhere to go that is not the human's tree, and `/tmp`
was denied. And `../scalpel`, which solves the same problem for Emacs, grants
`/tmp` and `/var/folders` read-write for exactly this reason, grants the git
configuration files a tool probes on every invocation, and keeps every other
extra grant an explicit, narrow, user-authored entry.

## Options considered

- Confine reads to the workspace on macOS and Linux too, so all three agree on
  the narrow reading: consistent, but it breaks reading a dependency's source,
  a system header, or a tool's own configuration -- work an agent does routinely.
- Leave Windows narrower and document the difference: honest, but a policy that
  means two things is a policy nobody can reason about.
- Grant the whole `/var/folders` tree as scalpel does: covers every child's
  TMPDIR, and hands over every other application's scratch with it.

## Decision

The sandbox bounds **writes**, not reads, and says so. Reads are open on all
three platforms; writes are the workspace plus the scratch roots (`$TMPDIR` and
`/tmp`, both spellings where `/var` is a symlink into `/private`). `ReadOnly`
still refuses every write, scratch included. The per-login TMPDIR leaf is
granted rather than all of `/var/folders`.

On Windows, parity is bought with an ACE: `FILE_GENERIC_READ` for the
AppContainer SID on the workspace's drive root and the system drive, merged into
the existing DACL, with the workspace and scratch granted read-write. There is
no path-based read rule in an AppContainer token, so the grant is the only way
to express it.

## Consequences

One policy means one thing everywhere, and an agent can write a script and run
it. The price is real and must not be glossed: **the Windows ACE is written to
the drive's security descriptor and outlives the process, the login and pearl
itself.** A `git revert` does not take it back. It is read-only and merged rather
than replacing, so it removes no existing access, but it is a machine-wide
change made by running a command.

Reads being open is also a disclosure surface: the agent can read a private key.
It cannot post it anywhere -- outbound network is denied -- but it can write it
into the workspace and let it reach the model as a tool result, which leaves by
the provider's connection rather than the shell's. Confining reads would close
that and break ordinary work; the stance here is the boundary-as-copy one
(ADR 5), where what the agent may see is the human's decision about where to run
it, not a cage the tool pretends to be.

Cost: a permanent read ACE on two drive roots per Windows machine, a disclosure
surface that is documented rather than closed, and a scratch grant that widens
every confined child's writable set beyond the workspace.