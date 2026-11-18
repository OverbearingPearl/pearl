# 6. Why the extension surface is advice, not a registry

Date: 2026-11-16

## Status

Amended by [21. Why the prompt has one mechanism and not a seam beside it](0021-why-the-prompt-has-one-mechanism-and-not-a-seam-beside-it.md)

## Context

Pi extensions register tools, commands, event handlers, renderers and
providers through an API object, and are distributed as npm or git packages.
That is one protocol both sides must keep implementing, in a runtime that
does not have one already.

## Options considered

- A registry, as in Pi: a manifest, a discovery step, a typed hook contract.
- The Emacs mechanisms: `advice-add`, `cl-defmethod`, and plain
  redefinition, plus three named variables for the parts that are data.

## Decision

Extensions attach the way Emacs extensions attach. Three seams exist because
they are data rather than behaviour: the tool list, the prompt function, and
the console mode hook.

## Consequences

A registry creates a second place where behaviour is defined, and the loop
stops being readable on its own: a reviewer has to know which handlers ran to
know what the loop did. Emacs already has a protocol for this, every user
knows it, and it needs no distribution story.

Cost: no discovery, so an installed extension is invisible until the user
loads it; no versioned hook contract, so an extension can break when the
shape of an argument changes; and advice can fail in ways a typed API would
have caught at compile time.
