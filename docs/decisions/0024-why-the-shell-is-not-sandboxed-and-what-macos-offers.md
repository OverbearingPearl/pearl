# 24. Why the shell is not sandboxed, and what macOS offers

Date: 2026-11-18

## Status

Accepted

## Context

ADR 5 refused a sandbox and said why: the three layers stop accidents, not
adversaries, and a shell command outside the snapshot is confirmed rather than
confined. That refusal is cheap to relitigate every time somebody notices that
"confirmed" means "a human decides at the moment the agent is most confident",
so the macOS half of it was measured rather than argued.

The measurements below were taken on macOS 27.2.1 with `sandbox-exec`, running
the shape a shell tool would use: `(allow default)`, `(deny file-write*)`, and
an allow list naming the tree. They are reproducible in a terminal; none of them
needs PEARL.

What was found:

- **It is deprecated, and has been since 2017.** `man sandbox-exec` on this
  machine prints `execute within a sandbox (DEPRECATED)`, dated March 2017, and
  points developers at App Sandbox, which needs an app bundle and entitlements
  and is not available to a command line tool.
- **It still works.** A write inside the allowed subpath succeeds; a write
  anywhere else fails with `Operation not permitted` and exit status 1, which is
  a result a tool can hand back to a model.
- **`/tmp` is a symlink to `/private/tmp`.** A profile that allows
  `/tmp/pearl-sbx` denies writes to `/tmp/pearl-sbx`, because the decision is
  made on the resolved path. Every path in a generated profile has to be
  canonicalised first, and the editor's own temporary directory is the one that
  makes this fail first.
- **`deny file-write*` also denies `/dev/null`.** The first command that reached
  for it was `git`, which died with `could not open '/dev/null' for reading and
  writing`. The profile has to allow `/dev/null` and the standard streams, which
  means the rule is not "writes only in the tree" but "writes only in the tree,
  plus a small list of exceptions that every program needs".
- **`TMPDIR` is outside the tree.** `mktemp -d` fails with
  `Operation not permitted`, so anything that builds through temporary files -
  most build tools - breaks unless the per-user temp directory is allowed too.
  That is a second write location the tree does not own.
- **Home-directory caches are denied, as intended.** Tools that cache under
  `~/.cache` or `~/Library` fail rather than leak, which is the trade working,
  and also the reason a sandboxed shell needs a per-tool story about caches.
- **One IPC probe stayed inside.** `osascript -e 'do shell script …'` from inside
  the sandbox was denied, so the obvious hand-off to an unsandboxed process did
  not escape this way. That is one probe and not an audit: the profile language
  is undocumented, unsupported, and has a public history of escapes.
- **It is cheap.** Thirty runs of `sh -c true`: 4.9 ms plain, 10.0 ms under
  `sandbox-exec`, so about 5 ms per invocation. Cost is not the obstacle.

## Options considered

- Use `sandbox-exec` on macOS, the way a tool that needs confinement would.
  Deprecated for nine years, profile language undocumented, and the measurements
  above show a profile that is both incomplete (it breaks `git` and `mktemp`)
  and, once completed, no longer a fence around the tree.
- Require `bubblewrap` on Linux and keep confirmation on macOS. Confinement
  where it is supported, an honest boundary where it is not.
- Refuse sandboxing entirely, keep ADR 5's confirmation regime, and record the
  measurements so the question can be answered instead of reopened.

## Decision

PEARL does not sandbox the shell. The confirmation regime and the proved write
scope stay as ADR 5 and ADR 16 describe them, and the confirmation prompt says
what it costs, because the action outranks the undo for exactly those commands.

If a sandbox is ever adopted it is `bubblewrap` on Linux only, behind a
capability check, with the confirmation regime as the fallback and the missing
binary named in the same list `pearl` already checks. Adopting it would amend
ADR 5 rather than extend it, because a sandbox changes what "confirmed" is for:
the point of one is not catching adversaries, it is turning a prompt into a
mechanism so that a write outside the tree needs no answer at all.

macOS gets no sandbox, and the reason is in the measurements: the only supported
mechanism is deprecated, and the profile that would confine writes to a project
directory has to allow `/dev/null`, the standard streams and the user's temp
directory to keep `git` and a build working - after which the remaining claim is
narrower than the sentence "sandboxed" suggests.

## Consequences

Nothing changes for a user except that the question is settled with evidence
instead of taste. A shell command outside the proved scope is still confirmed,
still not revertible, and the console still says so when it lands.

Cost: macOS users keep a human gate where Linux users could eventually get a
mechanism, and PEARL keeps an asymmetry between its two supported platforms that
will need a sentence in the manual when there is a manual. And this record has
to be re-measured before anyone relies on it: it describes one macOS release,
and the whole point of a deprecated tool is that the next release is allowed to
delete it.
