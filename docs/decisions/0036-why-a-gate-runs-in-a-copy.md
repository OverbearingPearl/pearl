# 36. Why a gate runs in a copy

Date: 2026-11-19

## Status

Accepted

## Context

ADR 33 gave every command a fingerprint of the tree it ran under, so a result
can say what moved while it ran. That answers "was this the same tree at the
end", not "what did the command read". A compiler that reads a file while
another session is halfway through writing it sees a state that is neither
version, and the exit status is then a fact about nothing anybody can name: the
fingerprints before and after differ, the drift names the path, and the verdict
is still in the result as if it meant something.

The tree is shared on purpose (ADR 3, ADR 22: one writer per session file, no
lock on the worktree), so a gate cannot wait for quiet, and it cannot ask the
other session to pause without inventing a lock the design refused two records
ago. What is left is to hand the gate a tree that cannot move: a copy taken
before the command starts.

Part 3 of ADR 25 is the neighbouring decision, not this one. There the copy
exists so an unnameable write becomes attributable and mergeable; here it exists
so a verdict names the text it read. The two will eventually share the copying
machinery, and they share it already in spirit: `cp -c` on APFS, a reflink copy
that costs metadata rather than blocks.

## Options considered

- Run the gate in the live tree and report the drift: this is what ADR 33 already
  does, and it names the change without saying which side of it the command read.
- Take a lock while a gate runs: it makes the tree quiet by stopping everybody
  else, which is the waiting the design refused.
- Trust the hook to run gates on a clean tree: it makes a verdict depend on a
  discipline nobody enforces.
- Copy the tree per gate: chosen.

## Decision

A command that is proved not to write may ask for a snapshot. The shell tool
takes the flag `:snapshot`; when it is set the tool copies the project root once,
runs the command with the copy as its directory, reads the epoch and the gate
note from the copy, and deletes the copy afterwards. A result produced this way
says so, because a reader who is shown output has to know which tree it is
about.

A command that writes is refused when it asks for a snapshot, by name: its
writes would land in a copy that is deleted, and a tool that silently discarded
them would be worse than one that says no. A directory without a repository is
refused too, because a snapshot is a tree, and a tree is what the epoch can
name.

The copy is a reflink where the filesystem has one (`cp -c -R` on macOS, `cp -a
--reflink=auto` elsewhere) and a plain recursive copy where it does not. The
choice is a performance accident, not a promise: a snapshot that cannot be
taken refuses the command rather than running it somewhere else.

## Consequences

A gate's verdict is about a tree that exists only as a name: the epoch of the
copy, carried in the note. A commit that cites it compares the text it stages
with the text the copy held (ADR 35), so two sessions editing different regions
of one file can both have their verdicts honoured, and the second session's
half-written text cannot make the first session's gate fail.

A command that reads absolute paths outside the root now reads them in the live
filesystem even under a snapshot, because only the root was copied. That is the
honest edge: the snapshot names the project, not the machine.

Cost: every snapshot gate pays for a copy of the root, so a gate on a large
repository is slower than one on the worktree, and the reflink only helps where
the filesystem offers one. A command whose output names its own working
directory shows the copy's path, which a reader has to be told about - the
result's sentence is that telling. The copy is deleted without inspection, so a
gate that wanted to leave something behind (a report, a build cache) loses it.
