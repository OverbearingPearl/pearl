;;; pearl-app.el --- the one place that knows every piece  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; A console is a buffer.  It draws, it takes text, and it must not know how
;; a session store is opened, which provider is in use or where the user's
;; instructions file lives.  This module knows all of that, once, so the
;; console can be replaced without touching any of it and every piece can be
;; handed to it already built.
;;
;; That is also why the wiring is here rather than in the console: product may
;; require core and adapters and the reverse is what the layer rule forbids, so
;; the module that assembles pieces from all three can only live up here.  A
;; console that assembled them would be the console depending on everything,
;; which is what `pearl-console-close' being unable to name a store already
;; showed.
;;
;; It reads files and it starts nothing.  The prompt's two files are read once,
;; here, and handed to the session as sources; the session composes the prompt
;; from them, so two consoles cannot rewrite each other's (ADR 20).  No
;; request is made from this module: a model call belongs to the loop.
;;
;; Planned API
;;
;; TODO `pearl-app-open' (&optional ROOT SUFFIX) -> app
;;      Create the console, the session and the loop for ROOT, and hand back
;;      the object that holds them.  SUFFIX names the session so two consoles
;;      on one root do not collide.  Nothing is opened yet: `pearl-app-wire'
;;      is where a file is touched, so a story can build an app and never
;;      write.
;;
;; TODO `pearl-app-console' (APP) -> console
;;      The buffer.  Everything the user does goes through it, so the app is
;;      reachable from a test without being a second thing the user types at.
;;
;; TODO `pearl-app-wire' (APP) -> APP
;;      Assemble the pieces the app is the only place able to assemble: open
;;      the session store with `:lock t', because a console is a writer and a
;;      second one on the same suffix is a conversation that answers itself
;;      (ADR 22); a store that is already locked becomes a `user-error' naming
;;      the session, saying whether the holder is this Emacs, and suggesting
;;      another suffix.  Then restore the session that reads it back, build the
;;      loop over `pearl-app-provider' and the tool list, install the default
;;      tools and the review wiring, pass the loop a guard closure over
;;      `buffer-live-p' (ADR 15), and attach undo to the turn events.  Every
;;      request the loop makes carries `(pearl-console-model console)', so the
;;      model is a property of the conversation and not of the provider.
;;
;;      It reads the prompt's files once with `pearl-app-prompt-sources' and
;;      hands them to the session through `pearl-session-set-prompt-sources'.
;;      It does not compose the prompt itself: the session does, from the
;;      sources it was given (ADR 20).
;;
;; TODO `pearl-app-provider' -> defcustom, a provider or nil
;;      The provider an app wires its loop over.  Nil builds the SSE provider
;;      from `pearl-provider-sse-base-url', the console's model and the key it
;;      resolved; a test binds a fake here, which is the second use of the
;;      interface ADR 7 asks for.
;;
;; TODO `pearl-app-user-instructions-file' -> defcustom, a file or nil
;;      The user's own rules, read for every console whatever the project is:
;;      the reply language, what to do before a commit, what never to touch.
;;      Defaults to `(locate-user-emacs-file "pearl/AGENTS.md")', and nil
;;      disables the layer, so someone whose rules are managed centrally can
;;      drop it without losing the project's (ADR 20).
;;
;; TODO `pearl-app-prompt-file-limit' -> defconst, in bytes
;;      How much of one instructions file reaches the prompt: 32 KiB, which is
;;      roughly what a long one costs in tokens.  A constant rather than a
;;      defcustom, because raising it is a different decision and not a
;;      setting: the prompt is paid for on every request, and the summary a
;;      compaction records is what pays it down.
;;
;; TODO `pearl-app-prompt-sources' (ROOT) -> list of sections
;;      Read the two files the app owns, in the order they belong in the
;;      prompt: `pearl-app-user-instructions-file' first, as `user', then
;;      `AGENTS.md' at ROOT as `project'.  A missing, empty or unreadable file
;;      is skipped rather than injected blank, and nothing above ROOT is read,
;;      because a parent's instructions are not this project's.  A file over
;;      `pearl-app-prompt-file-limit' is cut there and its text ends with a
;;      one-line marker saying it was cut, because a prompt that silently grew
;;      past the window fails later as a request error with no explanation.
;;      Called once when the app is wired, so one console has one prompt
;;      (ADR 20).

;;; Code:

(provide 'pearl-app)

;;; pearl-app.el ends here
