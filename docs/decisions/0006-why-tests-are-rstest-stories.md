# 6. Why tests are rstest stories

Date: 2026-11-24

## Status

Accepted

## Context

Every test is a Given-When-Then story that fails without the feature. The Elisp
PEARL vendors `ert-gwt` because it is not on MELPA. Rust needs an equivalent that
is lightweight and orthodox.

## Options considered

- A dedicated GWT crate: none exists on crates.io (`gwt` is a git-worktree CLI;
  `rstest-gwt` and `given-when-then` are absent).
- Vendor a bespoke framework: the Elisp route, but Rust already has a standard.
- `rstest` fixtures plus a `given_when_then` naming convention: rstest is the
  orthodox Rust fixture framework (155M downloads), a proc-macro with no runtime.

## Decision

We use `rstest` with `given_<state>_when_<action>_then_<outcome>` test names and
fixture injection. The GWT structure is a naming convention on top of rstest, not
a separate crate. Stories live in `crates/<unit>/tests/<unit>-tests.rs` and must
fail without the feature.

## Consequences

No vendored framework and no extra dependency beyond the standard; the story
shape is enforced by naming and review, mirroring the Elisp discipline.

Cost: the GWT shape is a convention, not mechanically enforced by a framework.
