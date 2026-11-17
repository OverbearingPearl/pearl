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
;;      Product may require core and adapters; the reverse is what
;;      the layer rule forbids, so the wiring has to live up here.
;;
;; TODO `pearl-console-provider' -> defcustom, a provider or nil
;;      The provider a console wires its loop over.  Nil builds the SSE
;;      provider from its own customs; a test binds a fake here, which is
;;      the second use of the interface ADR 7 asks for.
;;
;; TODO `pearl-console-mode' (MODE) -> mode definition
;;      The major mode of that buffer.  Enter sends, `C-c C-k' aborts, and the
;;      mode is what an extension hooks (ADR 6's third seam).
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

(provide 'pearl-console)

;;; pearl-console.el ends here
