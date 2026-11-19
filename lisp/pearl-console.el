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
;; `pearl-console-model-default' -> defcustom, a model id or nil
;;      The model a console asks for when nobody picked one in the buffer.
;;      Nil means ask once per console through `pearl-console-set-model';
;;      the answer is never written to the custom file, so a model chosen
;;      for one project does not follow the user to the next.
;;
;; `pearl-console-models' -> defcustom, list of model id strings
;;      The completions `pearl-console-set-model' offers.  It is a list and
;;      not a catalog: a wrong entry costs one rejected request, and a
;;      catalog is the thing ADR 7 says PEARL does not carry.
;;
;; `pearl-console-model' (CONSOLE) -> string
;;      The model this console sends.  The buffer-local choice wins, then
;;      `pearl-console-model-default', and a nil from both asks through
;;      `pearl-console-set-model' and remembers the answer for the buffer.
;;      Every request reads it, so changing it mid-conversation changes the
;;      next request and not the transcript.
;;
;; `pearl-console-set-model' (CONSOLE MODEL) -> string
;;      Set the console's model for this buffer: completing over
;;      `pearl-console-models' when MODEL is not given, storing it
;;      buffer-locally, and reporting the choice in the status line.  Bound
;;      to `C-c C-m', because the model is the one setting a user changes
;;      mid-session.
;;
;; `pearl-console-api-key' (CONSOLE) -> string
;;      The key this console sends, resolved through
;;      `pearl-provider-sse-api-key' once per console and kept in the
;;      buffer, so a second turn does not ask again and the value never
;;      reaches the session, an event or the status line (ADR 17).
;;
;; `pearl-console-mode' (MODE) -> mode definition
;;      The major mode of that buffer.  RET sends, `C-c C-k' aborts,
;;      `C-c C-m' picks the model, `C-c C-r' rolls back the turn under
;;      point, and the mode is what an extension hooks (ADR 6's third seam).
;;
;; `pearl-console-send' (CONSOLE TEXT) -> nil
;;      Admit one user message.  When a run is in flight the message steers
;;      instead of interrupting; the console decides which, once, and shows it.
;;
;; `pearl-console-render' (EVENT) -> list of sections
;;      The pure half: an event becomes a list of `(:text TEXT :role ROLE
;;      :fold FOLD)', where ROLE names a face and FOLD is an id when the
;;      section belongs to a foldable region.  No buffer, so a story asserts on
;;      the sections themselves (ADR 26).  The roles are the vocabulary:
;;      `user', `assistant', `tool', `status', `error', `fold', and the two the
;;      undo page adds, `timeline' and `timeline-other'.
;;
;; `pearl-console-face' (ROLE) -> face
;;      The one table from a role to a face, so the console never names a face
;;      inline.  Faces live in one `pearl-console' defgroup and are named after
;;      what happened, not after a colour: a user changes a theme rather than a
;;      formatter (ADR 26).
;;
;; `pearl-console-insert-event' (CONSOLE EVENT) -> nil
;;      The impure half: render the event, apply each section's properties
;;      (face from the role, the fold id, a help hint when the fold can be
;;      toggled, `rear-nonsticky' so the user's typing inherits nothing),
;;      update the state from the event, and schedule one redisplay.  It must
;;      be cheap: it runs once per delta.
;;
;; `pearl-console-state' (CONSOLE) -> symbol or nil
;;      What the console is doing, derived from events and never from a timer:
;;      `waiting' after a submit and before the first delta, `streaming' while
;;      deltas arrive, `tool' while a tool call runs, `confirm' while a question
;;      is open, `error' after a failure, nil when idle.  Each state has its own
;;      status word and face, because "nothing yet" and "a command is running"
;;      are different facts and a spinner can only say that time passed
;;      (ADR 26).
;;
;; `pearl-console-fold-toggle' (CONSOLE FOLD-ID &optional SHOW) -> boolean
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
;; `pearl-console-render-timeline' (NODES &optional OWN) -> sections
;;      The same sections `pearl-console-render' produces, one per node and in
;;      the order the nodes arrive: `:text' is the clock, the entry's id, its
;;      kind and what the kind added, `:role' is `timeline' for a node whose
;;      session is OWN and `timeline-other' for another session's, and `:fold'
;;      is the entry's id.  Pure, like its sibling: no buffer and no file, so a
;;      story asserts on the sections (ADR 26, ADR 30).
;;
;; `pearl-console-refresh' (CONSOLE NODES) -> number
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
;; `pearl-console-note' (CONSOLE NODES) -> nil
;;      A caller saw the session files change; the caller here is
;;      `pearl-console-watch'`s timer.  Insert when the page follows,
;;      the console is wired and the console is not `streaming', and count
;;      otherwise, which is all the mode line needs to carry a number instead
;;      of the nodes.  Point before the end of the output is the reader
;;      reading, so following stops there and the rows are counted until a
;;      send takes the page back.
;;
;; `pearl-console-watch' (CONSOLE) -> nil
;;      Make CONSOLE's timeline follow every session under its root.  The
;;      other console's writes arrive as a file this process never opened, so
;;      the page re-reads the directory on a timer
;;      (`pearl-console-follow-interval') instead of being pushed a row: one
;;      `file-attributes' per session per tick, and no entry read when none
;;      changed (ADR 32).  `pearl-console-unwatch' stops it, and closing the
;;      console does that for the caller.
;;
;; `pearl-console-follow' (CONSOLE &optional ON ON-P) -> boolean
;;      Whether the page follows appends.  On by default, and back on after a
;;      refresh.  It is turned off by the page itself the moment the user does
;;      something the page would disturb: opening a fold, asking for a plan,
;;      moving to a node, or leaving the tail.  Following is not a setting the
;;      user has to find, so the setter takes nil as an instruction.
;;
;; `pearl-console-pending' (CONSOLE) -> number
;;      How many nodes were appended while the page was not following.  It is a
;;      count of nodes and not of entries, because the page shows nodes.
;;
;; `pearl-console-status' (CONSOLE TEXT &optional FACE) -> nil
;;      Replace the status line.  The line is output, marked read-only and
;;      non-sticky, so text typed after it never inherits its properties.
;;      After a shell call the line names the active regime (M7): `project'
;;      when the write scope was proved inside the root, `confirm' when it
;;      could not be established.
;;
;; `pearl-console-abort' (CONSOLE) -> list of messages
;;      Abort the run and put the queue back in the editor, in the same order
;;      the loop handed it back.
;;
;; `pearl-console-rollback' (CONSOLE &optional ENTRY) -> list of paths
;;      Take a turn back after the fact: ask undo for the plan, confirm when
;;      it covers more than one path, and hand it to `pearl-undo-restore'.
;;      Before anything is written, say what it costs:
;;      which paths the plan will revert, which of them another session
;;      changed since the turn, and which will be refused.  A conflict is
;;      shown by name and nothing is written.  This is the only caller of the
;;      plan and the restore, and the command the console binds, because "undo
;;      that turn" is a user action and not a mechanic (ADR 23).
;;
;; `pearl-console-redraw' (CONSOLE) -> nil
;;      The batched half: apply appended text, fontify once, and move point only
;;      when the user was already at the end.  A user who scrolled up stays up.
;;
;; `pearl-console-close' (CONSOLE) -> nil
;;      Abort a run, close the session store, and kill the buffer.  Runs from
;;      `kill-buffer-hook', so it must tolerate a dead session.

;; `pearl-console-loop' (CONSOLE) -> loop
;;      The loop this console sends to, so a module that subscribes to the
;;      run's events does not have to look for it.  Nil before the console
;;      is wired.

;;; Code:

(require 'cl-lib)
(require 'seq)

(require 'pearl-contract)
(require 'pearl-event)
(require 'pearl-loop)
(require 'pearl-provider-sse)
(require 'pearl-session)
(require 'pearl-session-jsonl)
(require 'pearl-undo)

(defgroup pearl-console nil
  "The buffer a user talks to."
  :group 'pearl)

(defface pearl-console-user
  '((t :inherit font-lock-keyword-face))
  "What the user typed.")

(defface pearl-console-assistant
  '((t :inherit default))
  "What the model answered.")

(defface pearl-console-tool
  '((t :inherit font-lock-function-name-face))
  "What a tool did.")

(defface pearl-console-status
  '((t :inherit shadow))
  "What the console is doing, on the line that says so.")

(defface pearl-console-error
  '((t :inherit error))
  "A refusal, a failure, or a plan that will not apply.")

(defface pearl-console-fold
  '((t :inherit shadow))
  "The marker a folded region leaves behind.")

(defface pearl-console-timeline
  '((t :inherit default))
  "A row of the undo page that belongs to this conversation.")

(defface pearl-console-timeline-other
  '((t :inherit shadow))
  "A row of the undo page that belongs to another conversation.")

(defconst pearl-console--faces
  '((user . pearl-console-user)
    (assistant . pearl-console-assistant)
    (tool . pearl-console-tool)
    (status . pearl-console-status)
    (error . pearl-console-error)
    (fold . pearl-console-fold)
    (timeline . pearl-console-timeline)
    (timeline-other . pearl-console-timeline-other))
  "The one table from a role to a face (ADR 26).")

(defcustom pearl-console-model-default nil
  "The model a console asks for when nobody picked one in the buffer.
Nil asks once per console, and the answer stays in the buffer."
  :type '(choice (const :tag "ask" nil) string)
  :group 'pearl-console)

(defcustom pearl-console-models '("gpt-5" "gpt-5-mini" "o4-mini")
  "The completions `pearl-console-set-model' offers.
A list and not a catalog: a wrong entry costs one rejected request."
  :type '(repeat string)
  :group 'pearl-console)

(defcustom pearl-console-follow-interval 1.0
  "How often, in seconds, a timeline re-reads the sessions under its root.
A second console writes a file this process did not open, so the page polls
the directory instead of being told.  A tick costs one directory read and
one `file-attributes' per session file, and a tick that finds nothing new
reads no entry at all (ADR 32)."
  :type 'number
  :group 'pearl-console)

(defvar-local pearl-console--app nil
  "The app this console belongs to.")

(defvar-local pearl-console--session nil
  "The session this console reads and appends to.")

(defvar-local pearl-console--store nil
  "The record this console writes through.")

(defvar-local pearl-console--loop nil
  "The loop this console sends to.")

(defvar-local pearl-console--state nil
  "What the console is doing, or nil when it is idle.")

(defvar-local pearl-console--model nil
  "The model this buffer sends; the buffer-local choice wins.")

(defvar-local pearl-console--api-key nil
  "The key this buffer resolved, once (ADR 17).")

(defvar-local pearl-console--followed t
  "Whether the page follows what arrives.")

(defvar-local pearl-console--pending 0
  "How many nodes arrived while the page was not following.")

(defvar-local pearl-console--shown nil
  "The ids of the nodes this page already shows.")

(defvar-local pearl-console--folds nil
  "The fold ids this page has collapsed.")

(defvar-local pearl-console--timeline nil
  "Non-nil when this buffer is drawn as its root's timeline.")

(defvar-local pearl-console--signature nil
  "The size and time of the session files this page last read.")

(defvar-local pearl-console--watch-timer nil
  "The timer that re-reads this console's root, when one is running.")

(defconst pearl-console--fold-marker " … folded; TAB shows it\n"
  "What a folded section is drawn as, so the page says it is folded.")

(defvar-local pearl-console--regime nil
  "What the last tool result said about the regime it ran under (M7).")

(defvar-local pearl-console--status-text nil
  "The current status line.")

(defvar-local pearl-console--status-marker nil
  "Where the status line sits, so replacing it does not disturb output.")

(defvar-local pearl-console--output-end nil
  "Where output goes: before the status line and always at the end.")

(defvar-local pearl-console--input-start nil
  "Where the text the user is typing starts.")

(defvar-local pearl-console--closed nil
  "Non-nil once this console was closed, so closing twice is safe.")

(defvar-local pearl-console--redraw-pending nil
  "Non-nil when a redraw is already scheduled.")

(defvar-local pearl-console--review-unsubscribe nil
  "How to stop walking findings, when the console walks any.")

(defun pearl-console-face (role)
  "Return the face ROLE is drawn with."
  (or (alist-get role pearl-console--faces) 'default))

(defun pearl-console-render (event)
  "Return the sections EVENT is drawn as.
Each section is a plist of `:text', `:role' and, when it belongs to a
foldable region, `:fold' (ADR 26)."
  (let ((kind (plist-get event :kind))
        (data (plist-get event :data)))
    (pcase kind
      ('agent-start (list (list :text "pearl\n" :role 'status)))
      ('turn-start nil)
      ('message-start
       (let* ((message (plist-get data :message))
              (role (or (plist-get message :role) 'assistant)))
         (if (eq role 'user)
             (list (list :text (concat (or (plist-get message :content) "") "\n")
                         :role 'user))
           (list (list :text "" :role 'assistant)))))
      ('message-update
       (list (list :text (or (plist-get data :delta) "") :role 'assistant)))
      ('message-end
       (if (eq 'user (plist-get (plist-get data :message) :role))
           nil
         (list (list :text "\n" :role 'assistant :fold (plist-get data :id)))))
      ('tool-execution-start
       (list (list :text (format "\n%s " (plist-get data :name)) :role 'tool)))
      ('tool-execution-update
       (list (list :text (or (plist-get data :delta) "") :role 'tool)))
      ('tool-execution-end
       (list (list :text (pearl-console--result-text (plist-get data :result))
                   :role (if (plist-get (plist-get data :result) :error) 'error 'tool)
                   :fold (plist-get data :id))))
      ('turn-end nil)
      ('agent-end (list (list :text "\n" :role 'status)))
      (_ nil))))

(defun pearl-console--result-text (result)
  "Return the one line RESULT is shown as when it is not the point."
  (let ((content (or (plist-get result :content) "")))
    (cond
     ((plist-get result :error) (format "failed: %s\n" content))
     ((string-empty-p content) "done\n")
     (t (format "%s\n" (string-trim content))))))

(defun pearl-console--face-property (role)
  "Return the text property list ROLE is drawn with."
  (list 'face (pearl-console-face role)))

(defun pearl-console--insert-section (section)
  "Insert SECTION at the output end, folded when it carries a fold id."
  (let ((start (point)))
    (insert (or (plist-get section :text) ""))
    (add-text-properties start (point) (pearl-console--face-property
                                        (plist-get section :role)))
    (when-let* ((fold (plist-get section :fold)))
      (add-text-properties start (point) (list 'pearl-fold fold))
      (when (member fold pearl-console--folds)
        (add-text-properties start (point)
                             (list 'display pearl-console--fold-marker))))
    (add-text-properties start (point) '(rear-nonsticky t))))

(defun pearl-console-insert-event (console event)
  "Insert EVENT into CONSOLE, update its state, and schedule a redraw."
  (with-current-buffer console
    (setq pearl-console--timeline nil)
    (let ((at-end (= (point) (or pearl-console--output-end (point-max))))
          (inhibit-read-only t)
          (sections (pearl-console-render event)))
      (save-excursion
        (goto-char (or pearl-console--output-end (point-max)))
        (dolist (section sections)
          (pearl-console--insert-section section))
        (set-marker pearl-console--output-end (point)))
      (when at-end (goto-char (or pearl-console--output-end (point-max))))
      (pearl-console--absorb-event console event)
      (pearl-console--schedule-redraw console))))

(defun pearl-console--schedule-redraw (console)
  "Ask for one redraw of CONSOLE, and not one per delta."
  (with-current-buffer console
    (unless pearl-console--redraw-pending
      (setq pearl-console--redraw-pending t)
      (run-at-time 0 nil #'pearl-console--flush-redraw console))))

(defun pearl-console--flush-redraw (console)
  "Redraw CONSOLE once for everything that arrived since the last one."
  (when (buffer-live-p console)
    (with-current-buffer console
      (setq pearl-console--redraw-pending nil)
      (pearl-console-redraw console))))

(defun pearl-console--absorb-event (console event)
  "Read EVENT for what CONSOLE is doing, and update its status line."
  (let ((kind (plist-get event :kind))
        (data (plist-get event :data)))
    (pcase kind
      ('turn-start (pearl-console--set-state console 'waiting))
      ('message-update (pearl-console--set-state console 'streaming))
      ('message-start (pearl-console--set-state console 'streaming))
      ('tool-execution-start (pearl-console--set-state console 'tool))
      ('tool-execution-end (pearl-console--note-regime console
                                                      (plist-get data :result)))
      ('turn-end
       (pearl-console--set-state console nil)
       (pearl-console--restore-regime console))
      ('agent-end
       (pearl-console--set-state console nil)
       (pearl-console--restore-regime console))
      (_ nil))))

(defun pearl-console--restore-regime (console)
  "Put back into CONSOLE what the last tool result said about the regime."
  (when pearl-console--regime
    (pearl-console-status console pearl-console--regime)
    (setq pearl-console--regime nil)))

(defun pearl-console--note-regime (console result)
  "Write on the status line of CONSOLE which regime RESULT ran under (M7).
The answer outlives the answer that follows it: a run that ends with
prose would otherwise leave the status line saying a tool is streaming."
  (let ((scope (plist-get result :scope))
        (error (plist-get result :error))
        (text nil))
    (cond
     ((null result) nil)
     (scope
      (setq text (format "project: %s is inside the snapshot"
                         (mapconcat #'identity scope ", "))))
     ((and error (string-match-p "confirm\\|nobody" (format "%s" error)))
      (setq text "confirm: nothing was run, and nothing was written"))
     ((null (plist-get result :revertible))
      (setq text (concat "confirm: the write was not proved to stay inside"
                         " the project")))
     (t nil))
    (when text
      (setq pearl-console--regime text)
      (pearl-console-status console text))))

(defun pearl-console-fail (console reason)
  "Report that CONSOLE's run stopped, and REASON is why.

A failed run emits no `turn-end', so the state a send set would never be
cleared: without this the status line keeps waiting for a token the
endpoint has already refused."
  (with-current-buffer console
    (pearl-console--set-state console 'error)
    (pearl-console-status console (format "the run failed: %s" reason))))

(defun pearl-console-state (console)
  "Return what CONSOLE is doing, or nil when it is idle."
  (buffer-local-value 'pearl-console--state console))

(defun pearl-console--set-state (console state)
  "Record that CONSOLE is doing STATE, and say so once."
  (with-current-buffer console
    (unless (eq state pearl-console--state)
      (setq pearl-console--state state)
      (pcase state
        ('waiting (pearl-console-status console "waiting for the first token"))
        ('streaming (pearl-console-status console "streaming"))
        ('tool (pearl-console-status console "a tool is running"))
        ('error (pearl-console-status console "the run failed"))
        (_ nil)))))

(defun pearl-console-status (console &optional text face)
  "Return CONSOLE's status line, or replace it with TEXT drawn in FACE."
  (with-current-buffer console
    (if (null text)
        pearl-console--status-text
      (setq pearl-console--status-text text)
      (let ((inhibit-read-only t)
            (start (and pearl-console--status-marker
                        (marker-position pearl-console--status-marker))))
        (save-excursion
          (when (and start (<= start (point-max)))
            (goto-char start)
            (delete-region (point) (point-max)))
          (set-marker (or pearl-console--status-marker
                          (setq pearl-console--status-marker (make-marker)))
                      (point-max))
          (goto-char (point-max))
          (let ((from (point)))
            (insert "\n" text "\n")
            (add-text-properties from (point)
                                 (append (list 'face (or face
                                                         (pearl-console-face 'status)))
                                         '(read-only t rear-nonsticky t)))
            (set-marker pearl-console--output-end (1+ from)))))
      nil)))

(defun pearl-console--insert-user (console text)
  "Insert what the user said (TEXT) into CONSOLE, before the status line."
  (with-current-buffer console
    (let ((inhibit-read-only t)
          (at-end (= (point) (point-max))))
      (save-excursion
        (goto-char (or pearl-console--output-end (point-max)))
        (let ((start (point)))
          (insert text "\n")
          (add-text-properties start (point)
                               (append (pearl-console--face-property 'user)
                                       '(rear-nonsticky t)))
          (set-marker pearl-console--output-end (point))))
      (when at-end (goto-char (or pearl-console--output-end (point-max)))))))

(defun pearl-console-send (console text)
  "Admit TEXT into CONSOLE, steering when a run is already in flight."
  (with-current-buffer console
    (let ((loop pearl-console--loop)
          (message (pearl-contract-message 'user text)))
      (unless loop
        (user-error "This console was not wired, so there is nothing to send to"))
      ;; Sending is the reader saying they are done looking: the page goes
      ;; back to the tail, and what was counted while they read comes with
      ;; the next note.
      (setq pearl-console--followed t)
      (if (pearl-loop-busy-p loop)
          (progn (pearl-loop-steer loop message)
                 (pearl-console-status console "steering"))
        (pearl-console--insert-user console text)
        (pearl-loop-submit loop message)
        (pearl-console--set-state console 'waiting))
      nil)))

(defun pearl-console-abort (console)
  "Abort CONSOLE's run and return the messages it handed back."
  (with-current-buffer console
    (let ((queued (when pearl-console--loop (pearl-loop-abort pearl-console--loop))))
      (pearl-console--set-state console nil)
      (pearl-console-status console "aborted")
      ;; A message typed while the run was in flight was never drawn, so
      ;; it is drawn now: it is handed back to the user and has to be
      ;; somewhere they can see it.
      (dolist (message queued)
        (pearl-console--insert-user console (or (plist-get message :content) "")))
      queued)))

(defun pearl-console-fold-toggle (console fold &optional show)
  "Toggle the fold FOLD in CONSOLE, or set it to SHOW when SHOW is given.
A folded section is drawn as a marker rather than erased, and the fold is
recorded in the buffer: folding is a gesture, so the page stops following
while the reader is inside one (ADR 26, ADR 30)."
  (with-current-buffer console
    (let ((pos (text-property-any (point-min) (point-max) 'pearl-fold fold)))
      (when pos
        (let* ((visible (null (get-text-property pos 'display)))
               (next (if (null show) visible (and show t)))
               (end (next-single-property-change pos 'pearl-fold nil (point-max))))
          (with-silent-modifications
            (if next
                (add-text-properties pos end
                                     (list 'display pearl-console--fold-marker))
              (remove-text-properties pos end '(display nil))))
          (if next
              (push fold pearl-console--folds)
            (setq pearl-console--folds (delq fold pearl-console--folds)))
          (setq pearl-console--followed nil)
          next)))))

(defun pearl-console-render-timeline (nodes &optional own)
  "Return NODES as sections, saying which of them belong to OWN."
  (mapcar (lambda (node)
            (let* ((entry (plist-get node :entry))
                   (session (plist-get node :session))
                   (kind (plist-get entry :kind))
                   (data (plist-get entry :data))
                   (text (or (plist-get data :content) "")))
              (list :text (format "%s %s %s\n"
                                  (format-time-string "%H:%M:%S"
                                                      (plist-get entry :timestamp))
                                  kind
                                  (if (string-empty-p text) "" text))
                    :role (if (eq session own) 'timeline 'timeline-other)
                    :fold (plist-get entry :id))))
          nodes))

(defun pearl-console-follow (console &optional on on-p)
  "Return whether CONSOLE follows new rows, or set that to ON when ON-P.
A call with ON-P sets following, even to nil: that is how the page stops
following when the reader opens a fold or moves away from the tail, and
nil is the value the setter has to be able to write."
  (with-current-buffer console
    (if on-p
        (setq pearl-console--followed (and on t))
      pearl-console--followed)))

(defun pearl-console-pending (console)
  "Return how many nodes CONSOLE was told about but does not show."
  (buffer-local-value 'pearl-console--pending console))

(defun pearl-console--insert-nodes (console nodes)
  "Insert the NODES the buffer CONSOLE does not show yet, and count them."
  (let ((inserted 0))
    (dolist (node nodes)
      (let ((id (plist-get (plist-get node :entry) :id)))
        (unless (member id pearl-console--shown)
          (push id pearl-console--shown)
          (setq inserted (1+ inserted))
          (let ((section (car (pearl-console-render-timeline (list node)
                                                             (pearl-console--own console)))))
            (with-current-buffer console
              (let ((inhibit-read-only t))
                (save-excursion
                  (goto-char (point-max))
                  (pearl-console--insert-section section))))))))
    ;; The end of the output is where the rows now end: `pearl-console-note'
    ;; compares point against it to tell reading from following.
    (when (> inserted 0)
      (with-current-buffer console
        (set-marker pearl-console--output-end (point-max))))
    inserted))

(defun pearl-console--own (console)
  "Return the session CONSOLE owns, for the timeline's two roles."
  (buffer-local-value 'pearl-console--session console))

(defun pearl-console-refresh (console nodes)
  "Insert the NODES CONSOLE does not show, and turn following back on."
  (with-current-buffer console
    (let ((inserted (pearl-console--insert-nodes console nodes)))
      (setq pearl-console--pending 0)
      (setq pearl-console--followed t)
      (pearl-console-redraw console)
      inserted)))

(defun pearl-console-note (console nodes)
  "Insert NODES when CONSOLE follows, and count them otherwise.
The reader's position is the evidence: point before the end of the output
is somebody reading, so following stops there and the rows are counted
until a send takes the page back.  A gesture the console itself handled,
like a fold, says so in `pearl-console-fold-toggle'."
  (with-current-buffer console
    (when (and pearl-console--followed
               pearl-console--output-end
               (< (point) (marker-position pearl-console--output-end)))
      (setq pearl-console--followed nil))
    (if (and pearl-console--followed
             (not (eq 'streaming pearl-console--state))
             pearl-console--loop)
        (pearl-console-refresh console nodes)
      (let ((unseen 0))
        (dolist (node nodes)
          (unless (member (plist-get (plist-get node :entry) :id)
                          pearl-console--shown)
            (setq unseen (1+ unseen))))
        (setq pearl-console--pending unseen))))
  nil)

(defun pearl-console-redraw (console)
  "Fontify CONSOLE once and move point only when it was already at the end."
  (with-current-buffer console
    (let ((inhibit-read-only t))
      (font-lock-ensure))
    nil))

(defun pearl-console--plan-steps (plan)
  "Return the pieces of PLAN, each a `(patch . PATCH)' or `(restore . PATH)' pair."
  (mapcar (lambda (step)
            (if (plist-get step :file)
                (cons 'patch step)
              (cons (car step) (cadr step))))
          plan))

(defun pearl-console--plan-paths (plan)
  "Return the paths PLAN would write."
  (delete-dups (mapcar (lambda (pair)
                         (if (eq (car pair) 'patch)
                             (plist-get (cdr pair) :file)
                           (cdr pair)))
                       (pearl-console--plan-steps plan))))

(defun pearl-console--apply-patch (patch)
  "Reverse PATCH in its file, and return non-nil when it was applied.

Whether the written text is still there is `pearl-undo'`s question, not a
second copy of it: two searches for one anchor is how the copy drifts."
  (let* ((file (plist-get patch :file))
         (old (plist-get patch :old-text))
         (new (plist-get patch :new-text))
         (content (with-temp-buffer
                    (insert-file-contents file)
                    (buffer-string))))
    (if (pearl-undo-patch-conflict-p patch content)
        nil
      (let ((start (string-match (regexp-quote new) content)))
        (with-temp-file file
          (insert (concat (substring content 0 start)
                          old
                          (substring content (+ start (length new))))))
        t))))

(defun pearl-console-rollback (console &optional entry)
  "Take back, for CONSOLE, the turn ENTRY names; return the restored paths."
  (with-current-buffer console
    (let* ((session pearl-console--session)
           (plan (and session (pearl-undo-plan session entry)))
           (steps (pearl-console--plan-steps plan))
           (paths (pearl-console--plan-paths plan)))
      (cond
       ((null plan) nil)
       (t
        (if (and (< 1 (length paths))
                 (not (yes-or-no-p
                       (format "Restore %d files to what they were before the turn?"
                               (length paths)))))
            (progn (pearl-console-status console "rollback refused")
                   nil)
          (let ((restored nil)
                (conflict nil))
            (dolist (pair steps)
              (cond
               ((eq (car pair) 'patch)
                (let ((patch (cdr pair)))
                  (if (pearl-console--apply-patch patch)
                      (push (plist-get patch :file) restored)
                    (setq conflict (plist-get patch :file)))))
               ;; A pre-image is what the turn copied before a command it
               ;; could not prove wrote anywhere; restoring it needs no
               ;; snapshot, which is the whole reason it was taken.
               ;; A pre-image path is restored the same way a covered one
               ;; is: the snapshot is what knows what was there.  The
               ;; pre-image itself answers for paths no snapshot covers,
               ;; and `pearl-undo-preimage-restore' takes the record from
               ;; the entry, not from the plan.
               ((eq (car pair) 'preimage)
                (let ((snapshot (pearl-undo-latest (pearl-console--root console)
                                                   (pearl-console--suffix console))))
                  (condition-case nil
                      (progn (pearl-undo-restore snapshot (list (nth 1 pair)))
                             (push (nth 1 pair) restored))
                    (error (setq conflict (nth 1 pair))))))
               (t
                (let ((snapshot (pearl-undo-latest (pearl-console--root console)
                                                   (pearl-console--suffix console))))
                  (condition-case nil
                      (progn (pearl-undo-restore snapshot (list (cdr pair)))
                             (push (cdr pair) restored))
                    (error (setq conflict (cdr pair))))))))
            (cond
             (conflict
              (pearl-console-status console
                                    (format "not restored: %s changed since the turn"
                                            (file-name-nondirectory conflict)))
              nil)
             (t
              (when (and session pearl-console--store)
                (let ((undo (pearl-contract-entry 'undo (list :paths restored))))
                  (pearl-session-append session undo)
                  (pearl-session-jsonl-append pearl-console--store undo)))
              (pearl-console-status console
                                    (format "restored %d file%s"
                                            (length restored)
                                            (if (= 1 (length restored)) "" "s")))
              (nreverse restored))))))))))

(defun pearl-console--root (console)
  "Return the root CONSOLE writes under."
  (buffer-local-value 'pearl-console--root console))

(defun pearl-console--suffix (console)
  "Return the session suffix CONSOLE writes under."
  (buffer-local-value 'pearl-console--suffix console))

(defun pearl-console-loop (console)
  "Return the loop CONSOLE sends to, or nil before it was wired."
  (buffer-local-value 'pearl-console--loop console))

(defun pearl-console-mode-model (console &optional model ask)
  "Return the model of CONSOLE, set it to MODEL, or ask when ASK.

MODEL sets it.  ASK says a model nobody set may be asked for; without ASK
the answer is nil and the caller sends a request with no model, which is
what a background turn needs: a prompt in the middle of a turn takes the
minibuffer from whoever is using it."
  (with-current-buffer console
    (cond
     (model
      (setq pearl-console--model model)
      (pearl-console-status console (format "model: %s" model))
      model)
     (t
      (or pearl-console--model
          (setq pearl-console--model
                (or pearl-console-model-default
                    (and ask (pearl-console-set-model console)))))))))

(defun pearl-console-show-record (console entries)
  "Draw ENTRIES into CONSOLE, as the page a reopened session lands on.
The record is what is shown, not a replay: a message is its text and a
tool result is the line it was shown as, so reopening costs one pass and
no provider.

The page is drawn from scratch: a caller that moves the head and shows
the branch again must not leave the rows that are no longer on it."
  (with-current-buffer console
    (setq pearl-console--timeline nil)
    (let ((inhibit-read-only t))
      (erase-buffer)
      (when pearl-console--output-end
        (set-marker pearl-console--output-end (point-min)))
      (setq pearl-console--shown nil)))
  (dolist (entry entries)
    (let ((kind (plist-get entry :kind))
          (data (plist-get entry :data)))
      (pcase kind
        ('message
         (with-current-buffer console
           (let ((inhibit-read-only t))
             (save-excursion
               (goto-char (or pearl-console--output-end (point-max)))
               (let ((start (point)))
                 (insert (or (plist-get data :content) "") "\n")
                 (add-text-properties
                  start (point)
                  (append (pearl-console--face-property
                           (if (eq 'user (plist-get data :role)) 'user 'assistant))
                          '(rear-nonsticky t)))
                 (set-marker pearl-console--output-end (point)))))))
        ('tool
         (with-current-buffer console
           (let ((inhibit-read-only t))
             (save-excursion
               (goto-char (or pearl-console--output-end (point-max)))
               (let ((start (point)))
                 (insert (pearl-console--result-text data))
                 (add-text-properties
                  start (point)
                  (append (pearl-console--face-property
                           (if (plist-get data :error) 'error 'tool))
                          '(rear-nonsticky t)))
                 (set-marker pearl-console--output-end (point)))))))
        (_ nil))
      (push (plist-get entry :id) pearl-console--shown)))
  nil)

(defun pearl-console-model (console &optional ask)
  "Return the model CONSOLE sends, asking for one when ASK and none is set."
  (pearl-console-mode-model console nil ask))

(defun pearl-console-set-model (console &optional model)
  "Set CONSOLE's model to MODEL, asking for one when MODEL is not given."
  (with-current-buffer console
    (let ((chosen (or model
                      (completing-read "Model: " pearl-console-models nil nil))))
      (setq pearl-console--model chosen)
      (pearl-console-status console (format "model: %s" chosen))
      chosen)))

(defun pearl-console-api-key (console)
  "Return the key CONSOLE sends, resolving it once and keeping it here."
  (with-current-buffer console
    (or pearl-console--api-key
        (setq pearl-console--api-key
              (pearl-provider-sse-api-key
               (plist-get (pearl-provider-sse-endpoint
                           pearl-provider-sse-base-url)
                          :host))))))

(defun pearl-console--send-input ()
  "Send what the user typed at the prompt."
  (interactive)
  (let* ((console (current-buffer))
         (start (or pearl-console--input-start (point-min)))
         (text (string-trim (buffer-substring-no-properties start (point-max)))))
    (when (< 0 (length text))
      (pearl-console-send console text))))

(defun pearl-console--abort-command ()
  "Abort the run this console is showing."
  (interactive)
  (pearl-console-abort (current-buffer)))

(defun pearl-console--select-model ()
  "Pick the model this console sends."
  (interactive)
  (pearl-console-set-model (current-buffer)))

(defun pearl-console--sessions (console)
  "Return (SESSION . FILE) for every session under the root CONSOLE belongs to.
Readers do not lock (ADR 22): a session another console is writing is
read as far as its last complete line, which is what makes the page
usable while a run is in flight."
  (let* ((root (pearl-console--root console))
         (directory (expand-file-name "sessions/" root))
         (sessions nil))
    (when (file-directory-p directory)
      (dolist (file (directory-files directory t "\\.jsonl\\'"))
        (condition-case nil
            (let ((store (pearl-session-jsonl-open file)))
              (unwind-protect
                  (let ((entries (pearl-session-jsonl-read store)))
                    (when entries
                      (push (cons (pearl-session-restore entries) file) sessions)))
                (pearl-session-jsonl-close store)))
          (error nil))))
    (nreverse sessions)))

(defun pearl-console--signature (console)
  "Return the size and time of every session file under CONSOLE's root.
Cheap on purpose: a tick compares this and reads entries only when it
changed, so a root nobody is writing costs one `file-attributes' per
session per interval (ADR 32)."
  (let ((directory (expand-file-name "sessions/" (pearl-console--root console))))
    (when (file-directory-p directory)
      (mapcar (lambda (file)
                (let ((attributes (file-attributes file)))
                  (list file
                        (file-attribute-size attributes)
                        (file-attribute-modification-time attributes))))
              (directory-files directory t "\\.jsonl\\'")))))

(defun pearl-console--poll (console)
  "Read CONSOLE's root when its session files changed, and note the nodes.
Only a timeline has writers it does not own: a conversation buffer shows
one session and is drawn by its loop's events, so nothing here touches it."
  (when (buffer-live-p console)
    (with-current-buffer console
      (when pearl-console--timeline
        (let ((signature (pearl-console--signature console)))
          (unless (equal signature pearl-console--signature)
            (setq pearl-console--signature signature)
            (pearl-console-note
             console
             (pearl-undo-timeline
              (mapcar #'car (pearl-console--sessions console))))))))))

(defun pearl-console-watch (console)
  "Make CONSOLE's timeline follow every session under its root.
The other console's writes are a file this process never sees arrive, so
the page re-reads the directory on a timer rather than being pushed a
row: a poll is the one mechanism that behaves the same everywhere, and
what it costs is named by `pearl-console-follow-interval' (ADR 32).
Calling twice is a no-op, and `pearl-console-close' stops it."
  (with-current-buffer console
    (unless pearl-console--watch-timer
      (setq pearl-console--watch-timer
            (run-at-time pearl-console-follow-interval
                         pearl-console-follow-interval
                         #'pearl-console--poll console))))
  nil)

(defun pearl-console-unwatch (console)
  "Stop the timer CONSOLE's timeline follows with, if one is running."
  (with-current-buffer console
    (when pearl-console--watch-timer
      (cancel-timer pearl-console--watch-timer)
      (setq pearl-console--watch-timer nil)))
  nil)

(defun pearl-console-timeline (console)
  "Draw, into CONSOLE, every turn recorded under its root.

Every session is read the same way, so a change another console made is
on the page with no bookkeeping: the page is a projection of the files,
never a second record of them (ADR 30).  Returns the number of nodes
drawn."
  (interactive (list (current-buffer)))
  (let ((nodes (pearl-undo-timeline (mapcar #'car (pearl-console--sessions console)))))
    (with-current-buffer console
      (setq pearl-console--shown nil)
      (pearl-console-refresh console nodes)
      ;; The page is the root's now, so a writer this console does not own
      ;; belongs on it: the timer reads the files again when they change.
      (setq pearl-console--timeline t
            pearl-console--signature (pearl-console--signature console)))
    (length nodes)))

(defun pearl-console--suffixes (console)
  "Return the suffixes of every session under CONSOLE's root, as strings."
  (let ((root (pearl-console--root console)))
    (sort (mapcar (lambda (file)
                    (file-name-base file))
                  (and (file-directory-p (expand-file-name "sessions/" root))
                       (directory-files (expand-file-name "sessions/" root)
                                        t "\\.jsonl\\'")))
          #'string<)))

(defun pearl-console-switch-session (console &optional suffix)
  "Show the session SUFFIX of CONSOLE's root, in a console of its own.

An open console is one session and one file, so showing another one means
opening it: the buffer keeps the transcript of the session it was opened
for, and the record on disk is what a second console reads (ADR 29)."
  (interactive (list (current-buffer)))
  (let* ((root (pearl-console--root console))
         (suffix (or suffix
                     (completing-read "Session: " (pearl-console--suffixes console)
                                      nil nil)))
         (wanted (concat "*pearl: " suffix "*"))
         (existing (get-buffer wanted)))
    (if existing
        (progn (pop-to-buffer existing) existing)
      (let ((app (pearl-app-open root suffix)))
        (pearl-app-wire (pearl-app-console app))
        (pop-to-buffer (pearl-app-console app))))))

(defun pearl-console--switch-command ()
  "Ask for a session under this console's root and show it."
  (interactive)
  (pearl-console-switch-session (current-buffer)))

(defun pearl-console-checkout (console node)
  "Move CONSOLE's session to NODE, saying which writes it abandons.

The head is a pointer into an append-only record, so moving it deletes
nothing; what changes is what the next request reads.  The turns no longer
on the branch are named before the move, because a file they wrote stays
written, and a reader who moved back expects the disk to move with them
\(ADR 29)."
  (interactive (list (current-buffer) (pearl-console--node-at-point)))
  (let* ((session (plist-get node :session))
         (entry (plist-get node :entry))
         (plan (condition-case nil
                   (pearl-undo-plan session entry)
                 (error nil)))
         (files (seq-keep (lambda (step) (plist-get step :file)) plan)))
    (pearl-session-set-head session entry)
    (pearl-console-show-record console (pearl-session-branch session))
    (pearl-console-status
     console
     (if files
         (format "moved back; %s still written" (mapconcat #'identity files " "))
       "moved back"))
    (plist-get entry :id)))

(defun pearl-console--checkout-command ()
  "Move the session to the timeline node at point."
  (interactive)
  (let ((node (pearl-console--node-at-point)))
    (if node
        (pearl-console-checkout (current-buffer) node)
      (user-error "Point is on no entry of the timeline"))))

(defun pearl-console--branch-of (session branches)
  "Return the pair in BRANCHES that records SESSION, or nil.

The console re-reads every file for each gesture, so the session a row
carries is a different object from the one `pearl-console--sessions'
just restored; the entries are the same record, and their ids are what
names the branch."
  (let ((ids (mapcar (lambda (entry) (plist-get entry :id))
                     (pearl-session-entries session))))
    (seq-find (lambda (branch)
                (equal ids
                       (mapcar (lambda (entry) (plist-get entry :id))
                               (pearl-session-entries (car branch)))))
              branches)))

(defun pearl-console--entry-before (session entry)
  "Return the entry SESSION recorded just before ENTRY, or nil at the root."
  (let ((branch (pearl-session-branch session))
        (previous nil))
    (while (and branch (not (eq (car branch) entry)))
      (setq previous (car branch))
      (setq branch (cdr branch)))
    (when branch previous)))

(defun pearl-console-timeline-undo (console node)
  "Take back, in CONSOLE, the change NODE names, whichever branch wrote it.

The plan is built across sessions because the pointer is a branch's and
the view is the root's; the undo is recorded on the branch it undoes and
carries `:by', so that branch's own reader sees who asked (ADR 30).

The plan takes a branch's head back TO an entry, while this gesture names
the entry to take back FROM, so the entry the plan is built to is the one
before the row: a row is the turn's first change, and a plan that started
at the row would leave the row's own write in place."
  (interactive (list (current-buffer) (pearl-console--node-at-point)))
  (let* ((branches (pearl-console--sessions console))
         (own (plist-get node :session))
         (branch (pearl-console--branch-of own branches))
         (session (if branch (car branch) own))
         (id (plist-get (plist-get node :entry) :id))
         (entry (if branch
                    (seq-find (lambda (each) (equal id (plist-get each :id)))
                              (pearl-session-entries session))
                  (plist-get node :entry)))
         (before (and entry (pearl-console--entry-before session entry)))
         (sessions (mapcar #'car branches))
         (plan (pearl-undo-plan-at
                sessions (list :entry before :session session)))
         (file (cdr branch))
         (conflict nil)
         (restored nil))
    (dolist (step plan)
      (cond
       ((plist-get step :file)
        (if (pearl-console--apply-patch step)
            (push (plist-get step :file) restored)
          (setq conflict (plist-get step :file))))
       (t
        (let ((snapshot (pearl-undo-latest (pearl-console--root console)
                                           (file-name-base (or file "")))))
          (condition-case nil
              (progn (pearl-undo-restore snapshot (list (cadr step)))
                     (push (cadr step) restored))
            (error (setq conflict (cadr step))))))))
    (unless conflict
      (when session
        (let ((undo (pearl-contract-entry
                     'undo (list :paths (nreverse restored)
                                 :by (buffer-name console)))))
          (pearl-session-append session undo)
          ;; The undo is written to the branch it undoes, not to the
          ;; console's own file, so that branch's reader sees it in place.
          (when file
            (let ((store (pearl-session-jsonl-open file)))
              (unwind-protect
                  (pearl-session-jsonl-append store undo)
                (pearl-session-jsonl-close store)))))))
    (pearl-console-status
     console
     (if conflict
         (format "not restored: %s changed since then"
                 (file-name-nondirectory conflict))
       (format "restored %d file%s" (length restored)
               (if (= 1 (length restored)) "" "s"))))
    (nreverse restored)))

(defun pearl-console--node-at-point ()
  "Return the timeline node at point, or nil when point is on no node."
  (let ((id (get-text-property (point) 'pearl-fold))
        (nodes nil))
    (when id
      (setq nodes (pearl-undo-timeline
                   (mapcar #'car (pearl-console--sessions (current-buffer)))))
      (car (seq-filter (lambda (node)
                         (equal id (plist-get (plist-get node :entry) :id)))
                       nodes)))))

(defun pearl-console--timeline-command ()
  "Show every turn recorded under this console's root."
  (interactive)
  (message "PEARL: %d turns" (pearl-console-timeline (current-buffer))))

(defun pearl-console--timeline-undo-command ()
  "Take back the turn at point."
  (interactive)
  (let ((node (pearl-console--node-at-point)))
    (unless node (user-error "No turn at point"))
    (pearl-console-timeline-undo (current-buffer) node)))

(defun pearl-console-compact (console)
  "Ask the app behind CONSOLE to summarise and keep the tail."
  (interactive (list (current-buffer)))
  (let ((app (buffer-local-value 'pearl-console--app console)))
    (unless app (user-error "This console was not wired"))
    (pearl-app-compact app)))

(defun pearl-console--compact-command ()
  "Summarise what this console has recorded so far."
  (interactive)
  (pearl-console-compact (current-buffer)))

(defun pearl-console--rollback-command ()
  "Take the last turn back."
  (interactive)
  (pearl-console-rollback (current-buffer)))

(defun pearl-console--fold-command ()
  "Fold or unfold the section at point."
  (interactive)
  (let ((fold (or (get-text-property (point) 'pearl-fold)
                  (and (> (point) (point-min))
                       (get-text-property (1- (point)) 'pearl-fold)))))
    (unless fold
      (user-error "Point is on no foldable section"))
    (pearl-console-fold-toggle (current-buffer) fold)))

(defvar pearl-console-mode-map
  (let ((map (make-sparse-keymap)))
    (define-key map (kbd "RET") #'pearl-console--send-input)
    (define-key map (kbd "TAB") #'pearl-console--fold-command)
    (define-key map (kbd "C-c C-k") #'pearl-console--abort-command)
    (define-key map (kbd "C-c C-m") #'pearl-console--select-model)
    (define-key map (kbd "C-c C-r") #'pearl-console--rollback-command)
    (define-key map (kbd "C-c C-t") #'pearl-console--timeline-command)
    (define-key map (kbd "C-c C-u") #'pearl-console--timeline-undo-command)
    (define-key map (kbd "C-c C-o") #'pearl-console--checkout-command)
    (define-key map (kbd "C-c C-c") #'pearl-console--compact-command)
    (define-key map (kbd "C-c C-s") #'pearl-console--switch-command)
    map)
  "The keys the console binds, and no others.")

(define-derived-mode pearl-console-mode fundamental-mode "Pearl"
  "The major mode of a PEARL console.
RET sends, TAB folds the row at point, aborts with
\[pearl-console--abort-command], the model command picks a model and the
rollback command takes the last turn back.  The mode hook is the seam an
extension hooks (ADR 6)."
  (setq-local truncate-lines nil)
  (setq-local scroll-conservatively 101)
  (setq-local pearl-console--folds nil)
  (setq-local pearl-console--shown nil)
  (setq-local pearl-console--followed t)
  (setq-local pearl-console--pending 0)
  (setq-local pearl-console--status-text nil)
  (setq-local pearl-console--output-end (set-marker (make-marker) (point-min)))
  (setq-local pearl-console--input-start (point-max-marker))
  (add-hook 'kill-buffer-hook #'pearl-console-close nil t))

(defun pearl-console-close (&optional console)
  "Abort the run of CONSOLE, close its record and kill its buffer.
CONSOLE defaults to the current buffer, because this runs from
`kill-buffer-hook' and a hook takes no arguments.  Closing twice is a
no-op, and the hook is taken off before the kill so this cannot call
itself through it."
  (setq console (or console (current-buffer)))
  (when (and (buffer-live-p console)
             (not (buffer-local-value 'pearl-console--closed console)))
    (with-current-buffer console
      (setq pearl-console--closed t)
      (remove-hook 'kill-buffer-hook #'pearl-console-close t)
      (pearl-console-unwatch console)
      (when pearl-console--loop
        (condition-case nil (pearl-loop-abort pearl-console--loop) (error nil)))
      (when pearl-console--review-unsubscribe
        (condition-case nil
            (funcall pearl-console--review-unsubscribe)
          (error nil)))
      (when (and pearl-console--store pearl-console--app)
        (condition-case nil
            (pearl-app--record pearl-console--app)
          (error nil)))
      (when pearl-console--store
        (condition-case nil
            (pearl-session-jsonl-close pearl-console--store)
          (error nil)))
      (setq pearl-console--store nil)
      (kill-buffer console)))
  nil)

(defvar-local pearl-console--root nil
  "The project root this console writes under.")

(defvar-local pearl-console--suffix nil
  "The session suffix this console writes under.")

(defun pearl-console--open (app root suffix)
  "Open a console buffer for APP, under ROOT, named by SUFFIX."
  (let ((buffer (generate-new-buffer (format "*pearl: %s*" suffix))))
    (with-current-buffer buffer
      ;; The mode kills the buffer-local variables, so it is called before
      ;; any of them is set; setting them first is the same as not setting
      ;; them at all.
      (pearl-console-mode)
      (setq pearl-console--app app
            pearl-console--root root
            pearl-console--suffix suffix)
      (setq default-directory root))
    buffer))

(provide 'pearl-console)

;;; pearl-console.el ends here
