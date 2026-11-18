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
;; It composes nothing.  A console used to open the session store, build the
;; loop, resolve the API key and read the prompt's files, which made the buffer
;; the one module that knew every other module - and the module a replacement
;; surface could least afford to be without.  That is `pearl-app' now: this
;; module draws, sends, and asks the pieces it was handed.  The one thing it
;; still owns is the per-buffer choice of model, because that is a thing the
;; user changes while looking at the buffer.
;;
;; Planned API
;;
;; TODO `pearl-console-model-default' -> defcustom, a model id or nil
;;      The model a console asks for when nobody picked one in the buffer.
;;      Nil means ask once per console through `pearl-console-set-model';
;;      the answer is never written to the custom file, so a model chosen
;;      for one project does not follow the user to the next.
;;
;; TODO `pearl-console-models' -> defcustom, list of model id strings
;;      The completions `pearl-console-set-model' offers.  It is a list and
;;      not a catalog: a wrong entry costs one rejected request, and a
;;      catalog is the thing ADR 7 says PEARL does not carry.
;;
;; TODO `pearl-console-model' (CONSOLE) -> string
;;      The model this console sends.  The buffer-local choice wins, then
;;      `pearl-console-model-default', and a nil from both asks through
;;      `pearl-console-set-model' and remembers the answer for the buffer.
;;      Every request reads it, so changing it mid-conversation changes the
;;      next request and not the transcript.
;;
;; TODO `pearl-console-set-model' (CONSOLE MODEL) -> string
;;      Set the console's model for this buffer: completing over
;;      `pearl-console-models' when MODEL is not given, storing it
;;      buffer-locally, and reporting the choice in the status line.  Bound
;;      to `C-c C-m', because the model is the one setting a user changes
;;      mid-session.
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
;; TODO `pearl-console-send' (CONSOLE TEXT) -> nil
;;      Admit one user message.  When a run is in flight the message steers
;;      instead of interrupting; the console decides which, once, and shows it.
;;
;; TODO `pearl-console-render' (EVENT) -> list of sections
;;      The pure half: an event becomes a list of `(:text TEXT :role ROLE
;;      :fold FOLD)', where ROLE names a face and FOLD is an id when the
;;      section belongs to a foldable region.  No buffer, so a story asserts on
;;      the sections themselves (ADR 26).  The roles are the vocabulary:
;;      `user', `assistant', `tool', `status', `error', `fold', and the two the
;;      undo page adds, `timeline' and `timeline-other'.
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
;;      The keys that change what the page shows set following to nil before
;;      they act, which is what `pearl-console-follow' documents; this function
;;      is the view operation and not the gesture, so it stays testable without
;;      a keymap (ADR 30).
;;
;; TODO `pearl-console-render-timeline' (NODES &optional OWN) -> sections
;;      The same sections `pearl-console-render' produces, one per node and in
;;      the order the nodes arrive: `:text' is the clock, the entry's id, its
;;      kind and what the kind added, `:role' is `timeline' for a node whose
;;      session is OWN and `timeline-other' for another session's, and `:fold'
;;      is the entry's id.  Pure, like its sibling: no buffer and no file, so a
;;      story asserts on the sections (ADR 26, ADR 30).
;;
;; TODO `pearl-console-refresh' (CONSOLE NODES) -> number
;;      Insert the nodes the page has not shown yet, in timeline order, and turn
;;      following back on.  Returns how many rows went in.  This is `g`.
;;
;;      Everything a reader can be looking at is above the insertion or below
;;      it: nodes are only ever appended, so a refresh is an insertion and not
;;      a rebuild, and what is already on screen is neither refontified nor
;;      recentred and point does not move.  It is the console that inserts and
;;      the caller that read the files, because the console knows what a page
;;      looks like and does not know where a session lives (ADR 26, ADR 30).
;;
;; TODO `pearl-console-note' (CONSOLE NODES) -> nil
;;      A caller saw the session files change.  Insert when the page follows
;;      and the console is not `streaming', and count otherwise, which is all
;;      the mode line needs to carry a number instead of the nodes.
;;
;; TODO `pearl-console-follow' (CONSOLE &optional ON) -> boolean
;;      Whether the page follows appends.  On by default, and back on after a
;;      refresh.  It is turned off by the page itself the moment the user does
;;      something the page would disturb: opening a fold, asking for a plan,
;;      moving to a node.  Following is not a setting the user has to find.
;;
;; TODO `pearl-console-pending' (CONSOLE) -> number
;;      How many nodes were appended while the page was not following.  It is a
;;      count of nodes and not of entries, because the page shows nodes.
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
;;      Take a turn back after the fact: ask undo for the plan, confirm when
;;      it covers more than one path, and hand it to `pearl-undo-restore'.
;;      Before anything is written, say what it costs:
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
