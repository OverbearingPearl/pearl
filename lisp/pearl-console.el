;;; pearl-console.el --- the buffer the user talks to  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; One buffer: input at the end, output above it, one status line, and no
;; second window to keep in sync.  The console owns nothing the loop needs; it
;; subscribes to events and draws, and a different surface could replace it
;; without the loop noticing.
;;
;; Two rules make it usable rather than only correct.  Rendering is batched:
;; a stream of deltas appends text and schedules one redisplay, so a fast model
;; does not cost one redisplay per token.  And output is fontified once, at the
;; point the text lands, not on every redisplay of a buffer that grows.
;;
;; Planned API
;;
;; TODO `pearl-console-open' (&optional ROOT SUFFIX) -> buffer
;;      Create the buffer, the loop, and the session for ROOT; ask for a miss
;;      list once, when a tool the buffer needs is missing.  SUFFIX names the
;;      session so two consoles on one root do not collide.
;;
;; TODO `pearl-console-wire' (CONSOLE) -> CONSOLE
;;      Assemble the pieces the console is the only place able to assemble:
;;      open the session store, create the session that reads it back, build
;;      the loop over `pearl-console-provider' and the tool list, install the
;;      default tools and the review wiring, pass the loop a guard closure
;;      over `buffer-live-p' (ADR 15), and attach undo to the turn events.
;;      Every request the loop makes carries `(pearl-console-model console)',
;;      so the model is a property of the console and not of the provider.
;;      It reads the prompt's files once with `pearl-console-prompt-sources'
;;      and hands them to the session through
;;      `pearl-session-set-prompt-sources'.  It does not compose the prompt
;;      itself: the session does, from the sources it was given, so two
;;      consoles cannot rewrite each other's (ADR 20).
;;      Product may require core and adapters; the reverse is what
;;      the layer rule forbids, so the wiring has to live up here.
;;
;; TODO `pearl-console-provider' -> defcustom, a provider or nil
;;      The provider a console wires its loop over.  Nil builds the SSE
;;      provider from `pearl-base-url', this console's model and the key it
;;      resolved; a test binds a fake here, which is the second use of the
;;      interface ADR 7 asks for.
;;
;; TODO `pearl-model' -> defcustom, a model id string or nil
;;      The model a console asks for when nobody picked one in the buffer.
;;      Nil means ask once per console through `pearl-console-set-model';
;;      the answer is never written to the custom file, so a model chosen
;;      for one project does not follow the user to the next.
;;
;; TODO `pearl-models' -> defcustom, list of model id strings
;;      The completions `pearl-console-set-model' offers.  It is a list and
;;      not a catalog: a wrong entry costs one rejected request, and a
;;      catalog is the thing ADR 7 says PEARL does not carry.
;;
;; TODO `pearl-console-model' (CONSOLE) -> string
;;      The model this console sends.  The buffer-local choice wins, then
;;      `pearl-model', and a nil from both asks through
;;      `pearl-console-set-model' and remembers the answer for the buffer.
;;      Every request reads it, so changing it mid-conversation changes the
;;      next request and not the transcript.
;;
;; TODO `pearl-console-set-model' (CONSOLE MODEL) -> string
;;      Set the console's model for this buffer: completing over
;;      `pearl-models' when MODEL is not given, storing buffer-locally, and
;;      reporting the choice in the status line.  Bound to `C-c C-m',
;;      because the model is the one setting a user changes mid-session.
;;
;; TODO `pearl-console-api-key' (CONSOLE) -> string
;;      The key this console sends, resolved through
;;      `pearl-provider-sse-api-key' once per console and kept in the
;;      buffer, so a second turn does not ask again and the value never
;;      reaches the session, an event or the status line (ADR 17).
;;
;; TODO `pearl-console-mode' (MODE) -> mode definition
;;      The major mode of that buffer.  RET sends, `C-c C-k' aborts,
;;      `C-c C-m' picks the model, `C-c C-r' rolls back the turn under
;;      point, and the mode is what an extension hooks (ADR 6's third seam).
;;
;; TODO `pearl-user-instructions-file' -> defcustom, a file or nil
;;      The user's own rules, read for every console whatever the project is:
;;      the reply language, what to do before a commit, what never to touch.
;;      Defaults to `(locate-user-emacs-file "pearl/AGENTS.md")', and nil
;;      disables the layer, so someone whose rules are managed centrally can
;;      drop it without losing the project's (ADR 20).
;;
;; TODO `pearl-console-prompt-sources' (ROOT) -> list of sections
;;      Read the two files the console owns, in the order they belong in the
;;      prompt: `pearl-user-instructions-file' first, as `user', then
;;      `AGENTS.md' at ROOT as `project'.  A missing, empty or unreadable file
;;      is skipped rather than injected blank, and nothing above ROOT is read,
;;      because a parent's instructions are not this project's.  Called once
;;      when the console is wired, so one console has one prompt (ADR 20).
;;
;; TODO `pearl-console-send' (CONSOLE TEXT) -> nil
;;      Admit one user message.  When a run is in flight the message steers
;;      instead of interrupting; the console decides which, once, and shows it.
;;
;; TODO `pearl-console-insert-event' (CONSOLE EVENT) -> nil
;;      Append what the event renders and schedule a redisplay.  It must be
;;      cheap: it runs once per delta.
;;
;; TODO `pearl-console-status' (CONSOLE TEXT &optional FACE) -> nil
;;      Replace the status line.  The line is output, marked read-only and
;;      non-sticky, so text typed after it never inherits its properties.
;;      After a shell call the line names the active regime (M7): `project'
;;      when the write scope was proved inside the root, `confirm' when it
;;      could not be established.
;;
;; TODO `pearl-console-abort' (CONSOLE) -> list of messages
;;      Abort the run and put the queue back in the editor, in the same order
;;      the loop handed it back.
;;
;; TODO `pearl-console-rollback' (CONSOLE &optional ENTRY) -> list of paths
;;      Take a turn back after the fact: ask the session for the rollback
;;      plan, confirm when the plan covers more than one path, and hand it to
;;      `pearl-undo-restore'.  A conflict is shown by name and nothing is
;;      written.  This is the only caller of the plan and the restore, and the
;;      command the console binds, because "undo that turn" is a user action
;;      and not a mechanic.
;;
;; TODO `pearl-console-redraw' (CONSOLE) -> nil
;;      The batched half: apply appended text, fontify once, and move point only
;;      when the user was already at the end.  A user who scrolled up stays up.
;;
;; TODO `pearl-console-close' (CONSOLE) -> nil
;;      Abort a run, close the session store, and kill the buffer.  Runs from
;;      `kill-buffer-hook', so it must tolerate a dead session.

;;; Code:

(defvar pearl-console-provider nil
  "The provider a console wires its loop over; a story binds a fake.")

(provide 'pearl-console)

;;; pearl-console.el ends here
