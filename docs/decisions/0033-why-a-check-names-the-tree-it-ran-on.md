# 33. Why a check names the tree it ran on

Date: 2026-11-19

## Status

Accepted

## Context

A root can hold more than one session, and the sessions write the same
worktree (ADR 30, ADR 31).  Nothing in a command's result says which tree the
result is about, so three failures look alike:

- A runs `make test` while B is halfway through an edit.  The gate fails over
  a tree that never existed as a whole, and A reads the failure as its own.
- A runs `make test`, then B writes, then A commits.  The commit is proved by
  a green run over a different tree.
- A commits with `git add -A` while B's edit is half-written, and the commit
  carries B's unfinished work under A's message.

The two refusals PEARL already has do not reach these.  A patch is refused
when the file no longer carries its text (ADR 4, ADR 16), which is a statement
about a file and not about a gate or a commit.  A turn's snapshot makes a
write revertible, not a verdict current.  A session's record is written at the
end of its turn (ADR 22), so a write that is halfway through one is on disk
and in no record at all.

## Options considered

- A session record entry written before the write, as an intent.  Rejected:
  the transcript would carry half a turn, and ADR 22 says the record is what
  happened.
- A root-wide lock or a wait between sessions.  Rejected: it takes the editor
  hostage and the loop is single-threaded per buffer; a session that waits on
  another is a session that can deadlock.
- Watching the tree with `file-notify`.  Rejected on measurement: on this
  macOS build the kqueue backend accepts a watch and delivers nothing for a
  write, which is the worst failure shape available.
- Fingerprinting the tree from git: the head commit plus every path git
  reports as changed, each with its size and modification time.  Chosen.

## Decision

`pearl-tree-epoch` names a working tree: `(:head COMMIT :changed ((PATH SIZE
MTIME) ...))`, read with `git status --porcelain=v1 --untracked-files=all` and
`--no-optional-locks` so a read never waits on another session's index lock.
`pearl-tree-drift` compares two epochs and returns the paths that differ.  The
shell tool reads one before and one after the command it runs, carries both as
`:epoch` and `:drift`, and adds a sentence to the result's content when
anything moved; the exit status stays the command's.

The fingerprint is coarse on purpose.  It answers "did anything move", not
"was my change overwritten".  A refusal to write, undo or commit compares the
anchored patch, because two sessions may change different regions of one file
and both be right; refusing on "this file was touched" would refuse the safe
case.  That is also why the epoch is not a lock: it is a name a verdict can be
attached to, and the reader decides what to do about a stale one.

The shell tool is the second product module that names an adapter, and the
only one that does so outside the composition root.  It earns the exception: a
command moves the tree while it runs, so the tree has to be read around the
run, and routing that through `pearl-app` would make the app time somebody
else's command.

Deferred, and each one is its own story: gate verdicts as entries with an
epoch, so a commit can require a pass whose epoch is an ancestor of the tree it
commits; a commit surface that stages only the paths a plan names and refuses
when the text at the anchor is not the patch's; and a gate run in the `cp -c`
copy of ADR 25 so the check is immune rather than merely honest.

The epoch is read with the variables that name another repository removed
(`GIT_DIR`, `GIT_INDEX_FILE` and the rest).  `git` reads them before it reads
`-C`, so without the removal a run inside a hook would describe the caller's
repository and call the root it was given unchanged.

## Consequences

A command that crosses a change now says so, and names the paths, so a reader
can tell "my gate failed" from "my gate ran over somebody else's half-written
file".  The command's own writes appear in the same list; nothing here can tell
them apart from another session's, and the sentence says what it knows.

A result is bigger: an epoch is a commit id and a list of changed paths, and it
travels in the tool result and in the entry that records it.  A repository
whose status is large makes the epoch large, and `git status` is paid twice per
command.

Cost: one `git status` before and after every shell command, a result that can
say "the tree moved" without being able to say who moved it, and a refusal
that still has to be built (ADR 25 part 3, gate entries, commit staging)
before a stale verdict is prevented rather than reported.
