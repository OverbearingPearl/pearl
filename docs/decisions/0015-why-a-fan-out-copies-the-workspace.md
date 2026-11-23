# 15. Why a fan-out copies the workspace

Date: 2026-11-24

## Status

Accepted

## Context

A parent node can fan out into children that run at the same time. They share
the queue, the provider and the session directory, but until this record they
also shared one working tree, so the only fan-out that could not corrupt itself
was a sequential one.

Two children editing the same file collide silently: the second write wins and
the first child's reasoning now describes a file that says something else. Worse,
the shell baseline (ADR 13) is captured per workspace, so reverting one child's
op restores the whole tree to that child's before-image and throws away every
write its siblings made. A revert that destroys other agents' work is not a
revert.

The conflict has to be resolved before the child runs, because what a child will
touch is not knowable until it has touched it.

## Options considered

Forbidding conflicting writes, by refusing a child whose files overlap a running
one, requires knowing the file set in advance and turns a scheduling problem into
a prediction problem.

Staying sequential is what this milestone exists to remove; it was the state
before, not an alternative.

An overlay filesystem per child is the kernel's own copy-on-write and would be
cheap, but it exists on Linux only, and ADR 13 already refused it for the same
reason on macOS and Windows.

## Decision

Each child gets its own copy of the workspace, made when it is spawned, under the
session directory as `children/<child>/`. Its tools, its shell and its baseline
all act inside that copy, so a child's writes and reverts are its own by
construction rather than by agreement.

## Consequences

Children cannot collide, because they are not writing to the same files, and no
scheduling rule has to be trusted to keep that true. A child's writes survive as
long as its copy does, which is the run, and stay inspectable at a known path
without any merge step having been invented.

The settled result flows back to the queue as text; the files a child produced do
not, and merging a child's tree into the parent's is not part of this milestone.
A child that edits a file the parent is waiting on therefore reports what it did
rather than delivering it, which is honest and visibly incomplete.

Cost: a workspace-sized copy per child per fan-out, the same order as the
baseline capture ADR 13 already accepted, and children's edits must be carried
back by hand or by a later milestone that computes a real merge.