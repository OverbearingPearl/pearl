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
;;      open the session store with `:lock t', because a console is a writer
;;      and a second one on the same suffix is a conversation that answers
;;      itself (ADR 22); a store that is already locked becomes a `user-error'
;;      naming the session, saying whether the holder is this Emacs, and
;;      suggesting another suffix.  Then create the session that reads it back,
;;      build
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
;; TODO `pearl-console-prompt-file-limit' -> defconst, in bytes
;;      How much of one instructions file reaches the prompt: 32 KiB, which is
;;      roughly what a long one costs in tokens.  A constant rather than a
;;      defcustom, because raising it is a different decision and not a
;;      setting: the prompt is paid for on every request, and PEARL has no
;;      compaction to pay it down with.
;;
;; TODO `pearl-console-prompt-sources' (ROOT) -> list of sections
;;      Read the two files the console owns, in the order they belong in the
;;      prompt: `pearl-user-instructions-file' first, as `user', then
;;      `AGENTS.md' at ROOT as `project'.  A missing, empty or unreadable file
;;      is skipped rather than injected blank, and nothing above ROOT is read,
;;      because a parent's instructions are not this project's.  A file over
;;      `pearl-console-prompt-file-limit' is cut there and its text ends with
;;      a one-line marker saying it was cut, because a prompt that silently
;;      grew past the window fails later as a request error with no
;;      explanation.  Called once when the console is wired, so one console
;;      has one prompt (ADR 20).
;;
;; TODO `pearl-console-send' (CONSOLE TEXT) -> nil
;;      Admit one user message.  When a run is in flight the message steers
;;      instead of interrupting; the console decides which, once, and shows it.
;;
;; TODO `pearl-console-render' (EVENT) -> list of sections
;;      The pure half: an event becomes a list of `(:text TEXT :role ROLE
;;      :fold FOLD)', where ROLE names a face and FOLD is an id when the
;;      section belongs to a foldable region.  No buffer, so a story asserts on
;;      the sections themselves (ADR 26).  The roles are the vocabulary:
;;      `user', `assistant', `tool', `status', `error', `fold'.
;;
;; TODO `pearl-console-face' (ROLE) -> face
;;      The one table from a role to a face, so the console never names a face
;;      inline.  Faces live in one `pearl-console' defgroup and are named after
;;      what happened, not after a colour: a user changes a theme rather than a
;;      formatter (ADR 26).
;;
;; TODO `pearl-console-insert-event' (CONSOLE EVENT) -> nil
;;      The impure half: render the event, apply each section's properties
;;      (face from the role, the fold id, a help hint when the fold can be
;;      toggled, `rear-nonsticky' so the user's typing inherits nothing),
;;      update the state from the event, and schedule one redisplay.  It must
;;      be cheap: it runs once per delta.
;;
;; TODO `pearl-console-state' (CONSOLE) -> symbol or nil
;;      What the console is doing, derived from events and never from a timer:
;;      `waiting' after a submit and before the first delta, `streaming' while
;;      deltas arrive, `tool' while a tool call runs, `confirm' while a question
;;      is open, `error' after a failure, nil when idle.  Each state has its own
;;      status word and face, because "nothing yet" and "a command is running"
;;      are different facts and a spinner can only say that time passed
;;      (ADR 26).
;;
;; TODO `pearl-console-fold-toggle' (CONSOLE FOLD-ID &optional SHOW) -> boolean
;;      Toggle or set what a fold shows.  Folds are view state: the id comes
;;      from the event, the state lives in the buffer, and the session is not
;;      touched.  A fold is always addressable, by a key and a hint on the
;;      marker, because an implicit collapse nobody can undo is a fold the user
;;      cannot read (ADR 26).
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
;;      `pearl-undo-restore'.  Before anything is written, say what it costs:
;;      which paths the plan will revert, which of them another session
;;      changed since the turn, and which will be refused.  A conflict is
;;      shown by name and nothing is written.  This is the only caller of the
;;      plan and the restore, and the command the console binds, because "undo
;;      that turn" is a user action and not a mechanic (ADR 23).
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
