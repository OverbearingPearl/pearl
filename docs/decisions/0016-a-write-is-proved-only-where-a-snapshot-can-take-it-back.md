# 16. A write is proved only where a snapshot can take it back

Date: 2026-11-17

## Status

Accepted

## Context

ADR 5's third layer proves a command's write scope at the project root
and confirms where the scope cannot be established. But root membership
is not revertibility: a gitignored file inside the root — `.env` is the
canonical example — is outside the turn snapshot, so a provably in-root
write to it passes the gate and no layer can take it back. The accident
the promise exists to stop walks straight through the net, and the
audit found it by asking what happens when the agent helpfully rewrites
the environment file.

## Options considered

- An exception list of important ignored files: a denylist that rots, and misses the next important file.
- Force ignored files into the snapshot: drags build trees and dependency directories into every turn, and the cost explodes.
- Tighten the regime's condition: a write is proved only where the snapshot can take it back, and a write to an ignored path selects the confirming regime even inside the root.

## Decision

`pearl-tools-shell-scope` returns nil for a command whose provable
writes include a gitignored path, even inside the project root. Nil
selects the confirming regime exactly as before; the status word
`project` now means in-root-and-covered. Outside a git repository there
is no snapshot at all, so no write is proved there either: a command
that writes nothing still runs, and a command that writes asks. ADR 5
is amended, not reversed — confirmation still happens exactly where
revertibility cannot reach, which is its own principle applied without
the root shortcut.

## Consequences

The gate now asks exactly where the safety net ends. The stories pin
both halves: an ignored write's scope is nil and unattended it is
refused with the file untouched, and a non-git root proves no write
scope at all.

Cost: a non-git project confirms every shell write, and a provably
harmless write to an ignored scratch file asks too. The regime is only
as good as the snapshot, and a user who wants fewer prompts initializes
git — which is also the thing that makes the second layer exist.
