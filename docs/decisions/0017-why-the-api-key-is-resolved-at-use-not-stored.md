# 17. Why the API key is resolved at use, not stored

Date: 2026-11-18

## Status

Accepted

## Context

PEARL needs one credential, for the one provider it implements (ADR 7). Where
that credential lives decides two things at once: whether someone who never
read the source can start a console with a key they already have, and whether
the key ends up in the places a key must not be — an init file tracked in a
dotfiles repository, a backup, a session transcript, a log line.

A `defcustom` is the obvious place and the wrong one. The custom file is
plaintext, it is often versioned, and `customize` prints the value in a
readable field. The session file is worse: PEARL records what happened, and a
key is not something that happened to the project.

## Options considered

- A `defcustom` in the custom file: one screen to set up, and the key is
  plaintext in whatever the user versions their Emacs config with.
- Prompt on every console open and keep it in a defvar: nothing at rest, but
  every Emacs start asks again, and a batch or remote run has nobody to ask.
- Resolve it where Emacs already keeps credentials, with a fallthrough: the
  key is named once in `auth-source` or the environment, and a prompt is the
  last resort rather than the first interface.

## Decision

`pearl-provider-sse-api-key` resolves in one order and never writes:

1. `auth-source` for the host named in the provider's base URL, so a user who
   already configured `~/.authinfo.gpg`, `pass` or the macOS keychain uses the
   key they already have;
2. the `PEARL_API_KEY` environment variable, which is what a container, CI or
   `env`-wrapped start gives a process that has no interactive prompt;
3. one `read-passwd` prompt, memoized for the session under that host, so a
   second turn in the same Emacs does not ask twice.

A miss signals `pearl-api-key-missing` naming the host and the two lookups
instead of sending an empty bearer, because an unauthorised request reports
itself as a 401 much later and much less clearly. The key reaches the wire in
exactly one place — the header builder — and the session, the events and the
status line never see it. The HTTP status is reported by name for the same
reason: the first thing a wrong key produces is a 401, and the story pins that
the adapter says so.

The model is the other half of the same setup screen and is not a secret, so
it is not this record's subject: `pearl-console-set-model` picks it and
nothing remembers it across an Emacs restart.

## Consequences

The key has no path into PEARL's own files: not the custom file, not the
JSONL record, not an event payload. A user's existing credential store starts
working on the first request, and a headless run needs only `PEARL_API_KEY`.
`auth-source` is a dependency of Emacs, not a package, so nothing new is
vendored.

Cost: a user who keeps the key nowhere those three lookups see must add an
`authinfo` line or export the variable before the first request, and the
prompt path cannot be exercised from a batch session. A mistyped key is only
diagnosed when the endpoint answers, which is why the status code is part of
the error the adapter reports rather than a log line.
