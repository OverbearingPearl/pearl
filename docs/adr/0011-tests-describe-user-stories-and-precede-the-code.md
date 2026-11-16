---
title: "ADR-0011: Tests describe user stories and precede the code"
date: "2026-11-16"
status: accepted
summary: >-
  Every test is a Given-When-Then story in the ert-gwt style, written before the module; make test may be red while make lint stays green.
---

# Context

The acceptance criteria in the roadmap were written as things a person can
run: an edit becomes exactly one `C-/`, a failing build is navigable without
leaving the console, kill Emacs and the history is there. Each one is a story
about what the user does, not about what a function returns.

An ERT case named after the function under test cannot state that. Its name
encodes the scenario, and the code tends to be written first and the test
shaped around it.

# Options considered

- Unit tests per function, written around the implementation.
- Behaviour tests in a separate BDD framework (Buttercup, Ecukes).
- Given-When-Then structure inside ERT, with the story as the test.

# Decision

Every test is a story in `ert-gwt` form, in `test/`, one file per module,
named after the module with a `-tests` suffix. Tests are written before the
module they exercise, and the roadmap's acceptance criterion for a stage is
the story that proves it.

# Consequences

`make test` is allowed to be red while a stage is being built; `make lint`
must be green at every commit. A test that cannot be written as a story is a
sign that the feature has no user-visible outcome yet.

The test name is generated from the story's content, so editing one story
never renames another, and no vocabulary of invented names accumulates.

Cost: `ert-gwt` is not yet on MELPA, so it is vendored under `vendor/` and
must be removed when the package lands there. A story's `:when` is exactly
one action, which occasionally forces a story to be split that a unit test
would have kept as one case.
