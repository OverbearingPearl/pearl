# 14. Vendor the test framework, and give vendor a class

Date: 2026-11-17

## Status

Accepted

## Context

Every test in this repository is a Given-When-Then story (ADR 11), so the
suite needs `ert-gwt`. It is not on MELPA yet, and a test run that needs the
network is a test run that fails for a reason nobody is working on.

The check suite already classifies modules by directory, and a directory it
does not know is a directory it silently ignores — which is how a vendored
copy turns into code nothing watches.

## Options considered

- Depend on the package from MELPA, and wait until it is there.
- Put a copy in `test/`, where the suite would lint it like our own file.
- Vendor one file under `vendor/`, and teach the suite that `vendor/` is a
  class.

## Decision

One file, `vendor/ert-gwt/ert-gwt.el`, kept byte for byte under its own
license and header, with a `README.md` beside it naming the upstream commit
and the condition for deleting it.

`vendor/` is a class in the check suite: discovered, classified, and put on the
load path (the directory and each package directory under it) for both lint and
`make test`. Its profile requires no header and no Commentary, forbids none,
and allows it to require only other vendored modules. Every per-file check
defaults to `main`, `business`, `test` and `tooling`, so no style rule of ours
reaches into a vendored file; whitespace hygiene still applies, because that
check runs over the whole tree.

## Consequences

`make test` runs offline, and a copy of somebody's package cannot quietly grow
a dependency on our own modules — the one rule the class carries is the one
that keeps the copy replaceable.

Cost: the copy has to be updated by hand, and `vendor/` is the kind of
directory that becomes a dumping ground once it exists. Both are answered by
policy rather than by tooling: AGENTS.md says the directory holds the test
framework and nothing else, and a second dependency needs a record like this
one first.

What would change our mind: `ert-gwt` on MELPA. Then delete the directory,
delete the class's only member, and declare the dependency the way the other
development tools are declared.
