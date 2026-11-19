;;; pearl-delegate.el --- A delegate is a session of its own  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;; This file is not part of GNU Emacs.

;;; Commentary:

;; A delegate is the one tool that does not answer from the tree it was
;; called in.  It starts a session of its own, with its own file and its
;; own lock (ADR 22), lets it run to a stop, writes what came out to a
;; file, and hands the parent a pointer of at most five hundred
;; characters (ADR 31).
;;
;; The size of that pointer is the point of the arrangement.  A model that
;; reads a report inline pays for the whole report on every later turn; a
;; model that gets a path and a paragraph pays for the paragraph, and asks
;; for the file when it wants the rest.  So nothing here splices a child's
;; transcript into a parent's record, and the parent's projection never
;; changes shape because a child read something.
;;
;; A read-only role is a session file, a loop and a branch, and nothing
;; else.  It writes no project file, so it adds nothing to the undo domain
;; and cannot touch a buffer the user is working in.  A worker would write,
;; and a worker is therefore refused until a parent's plan can expand
;; another session's plan by name; that is the whole reason the roles are
;; ordered this way.
;;
;; A delegate may not ask.  A confirmation from a background loop takes the
;; minibuffer from a user who is typing in it, and a fleet of them takes it
;; repeatedly, so a child either proceeds under its role or fails and says
;; why.

;; Planned API

;; TODO `pearl-delegate-role' (NAME) -> plist
;;      What NAME may do: `:tools' (the tool names the child may run),
;;      `:writes' (non-nil when the child may write the project) and
;;      `:asks' (nil, always, because a delegate may not ask).  A name this
;;      package does not know is not a role: it signals
;;      `pearl-tool-invalid' rather than guessing a tool list, because a
;;      guessed list is a permission nobody granted.

;; TODO `pearl-delegate-declaration' () -> declaration
;;      The `delegate' tool as a model sees it: a role and a task, both
;;      required.  There is no argument for a model, a provider or a
;;      budget: the child runs on the parent's provider (a second transport
;;      is a second set of failures) and a model choosing how many agents
;;      to start is a budget nobody set.

;; TODO `pearl-delegate-start' (ARGUMENTS CONTEXT) -> handle
;;      Start the child and return what it takes to wait for it: `:id',
;;      `:session', `:loop', `:file' and `:store'.  The session is written
;;      as it runs, so a parent killed mid-run leaves a child that can be
;;      read afterwards.

;; TODO `pearl-delegate-pointer' (TEXT) -> string
;;      The first paragraph of TEXT, cut at
;;      `pearl-delegate--pointer-limit' on a word boundary when one is
;;      near.  This is what the parent keeps; the rest stays in the file.

;; TODO `pearl-delegate-run' (ARGUMENTS CONTEXT) -> tool result
;;      The tool body: start, wait for the child to stop, write its text to
;;      `<root>/delegations/<id>.md', and answer with the pointer, the path
;;      and `:delegate' naming the child's session so a plan can find it
;;      later.  A refused role, a missing task or a child that failed are
;;      error results, because a tool never signals at the loop.

;; TODO `pearl-delegate-install' (REGISTRY) -> registry
;;      REGISTRY with the delegate tool added once.

;;; Code:

(provide 'pearl-delegate)

;;; pearl-delegate.el ends here
