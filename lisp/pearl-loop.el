;;; pearl-loop.el --- the double loop and its two queues  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The control flow, and the reason this package exists.  An inner loop runs
;; while there are tool calls or steering messages; an outer loop re-enters it
;; when follow-up messages are waiting.  The two channels differ in when they
;; may arrive, not in what they contain, and getting that wrong splits a turn
;; in half.
;;
;; The loop knows nothing about buffers, files or provider names.  It receives
;; a provider, a tool list, a session and an emitter, and it hands back what
;; happened; a surface can be replaced without the loop noticing.  The pieces
;; of a response are somebody else's business too: `pearl-turn' assembles a
;; text delta and a fragment of arguments into a message and a call, so this
;; module never parses a stream.
;;
;; Every wait is a callback, and callbacks arrive from process sentinels.  The
;; three rules in ADR 10 are implemented here and nowhere else: defer each step
;; to a zero-delay timer, check the run's generation before each dispatch, and
;; report a dead buffer instead of dropping the chain.
;;
;; Planned API
;;
;; `pearl-loop-new' (&key PROVIDER TOOLS SESSION EMIT ON-ERROR GUARD)
;;      -> loop
;;      PROVIDER answers requests, TOOLS is the executable tool list, SESSION
;;      is the record the projection reads.  EMIT is the emitter the run
;;      speaks through; nil means one is made here, and `pearl-loop-emitter'
;;      hands it back.  TOOLS may change between turns;
;;      the change is declared to the model as a difference, per ADR 8.
;;      GUARD is a function of no arguments, called before every dispatch;
;;      nil means the run's owner is gone (ADR 15): the chain stops through
;;      the terminal callback, the busy flag is released, and ON-ERROR
;;      receives a `pearl-loop-dead-owner' condition.  The loop knows a
;;      predicate, never a buffer.
;;
;; `pearl-loop-submit' (LOOP PROMPT) -> run id
;;      Admit one user message and start a run.  Signals `pearl-loop-busy'
;;      when a run is in flight; the caller queues instead of interrupting.
;;
;; `pearl-loop-steer' (LOOP MESSAGE) -> nil
;;      Deliver MESSAGE after the current turn's tool calls and before the
;;      next request.  The only channel that may arrive mid-run.
;;
;; `pearl-loop-follow-up' (LOOP MESSAGE) -> nil
;;      Deliver MESSAGE when the loop would otherwise stop.  It re-enters the
;;      inner loop as a new outer iteration and never splits a turn.
;;
;; `pearl-loop-abort' (LOOP) -> list of messages handed back
;;      Stop the run, keep the tool results already produced, and return the
;;      queue to the caller.  Borrow: pi's abort semantics, pinned in ADR 8.
;;
;; `pearl-loop-busy-p' (LOOP) -> boolean
;; `pearl-loop-queued' (LOOP &optional KIND) -> list of messages
;;      KIND is `steering', `follow-up', or nil for both in delivery order.
;;
;; `pearl-loop-emitter' (LOOP) -> emitter
;;      The emitter `:emit' was given, so a listener subscribes with
;;      `pearl-event-subscribe' and there is one subscription API in the
;;      package instead of two.  A loop built without `:emit' makes its own, so
;;      a story can watch a run without wiring a console.
;;
;; `pearl-loop-set-tools' (LOOP TOOLS) -> nil
;;      Replace the executable tool list between turns.  The change is
;;      declared to the model as a difference on a system message before
;;      the next request, per ADR 8, so replaying the transcript yields
;;      the same tool set.
;;
;; `pearl-loop--dispatch' (LOOP STEP) -> nil
;;      Schedule STEP on a zero-delay timer when the generation still matches.
;;      Borrow: scalpel's deferred-dispatch loop, `scalpel-agent-run', which is
;;      where the three rules in ADR 10 were worked out; the generation
;;      counter and the killed-buffer report are both its.  This comment is
;;      scaffolding: the code that lands here replaces it, and the record in
;;      ADR 10 says what survived.
;;
;; `pearl-loop--record-message' (LOOP MESSAGE) -> entry
;;      Append one entry to the session and make it the new head: the
;;      assistant message, a system message, or a tool result carrying the
;;      patch its tool returned.  The loop writes; the projection reads.  This
;;      is the edge the memory ring closes on, and it is why the session is a
;;      core module rather than a subscriber's business (ADR 3).
;;
;; `pearl-loop--record-result' (LOOP CALL RESULT) -> entry
;;      Append the tool result for CALL, carrying the patch it returned.
;;
;; `pearl-loop--turn' (LOOP) -> nil
;;      One assistant response plus the tool calls it makes.  The deltas are
;;      not read here: a `pearl-turn' accumulates them, and this asks it for
;;      the message, the calls and the stop reason, which is the whole of the
;;      loop's interest in the wire.  A `length' stop fails every tool call of
;;      the turn instead of running truncated ones, and a failed tool call is a
;;      result, never a signal: a call whose arguments did not parse fails by
;;      name while the turn continues.
;;      Borrow: pi's runLoop, pinned in ADR 8.

;;; Code:

(require 'cl-lib)
(require 'seq)
(require 'pearl-contract)
(require 'pearl-event)
(require 'pearl-provider)
(require 'pearl-session)
(require 'pearl-tool)
(require 'pearl-turn)

(define-error 'pearl-loop-busy "A run is already in flight")

(define-error 'pearl-loop-dead-owner "The run's owner is gone" 'quit)

(cl-defstruct (pearl-loop (:constructor pearl-loop--create))
  "One agent loop and the two queues that feed it.

`state' is `running' or `idle' and nothing else; `generation' counts runs
so a callback from an abandoned one can tell it is stale."
  provider tools session emit guard on-error
  (state 'idle) (generation 0) (run 0) (handle nil) (turn nil) (finished nil)
  (started nil) (steering nil) (follow-ups nil))

(cl-defun pearl-loop-new (&key provider tools session emit guard on-error)
  "Build a loop over one provider, one session and one tool list.

PROVIDER answers requests, TOOLS is the executable tool list, SESSION is
the record the projection reads, EMIT is the emitter the run speaks
through, GUARD is the owner predicate, and ON-ERROR receives a condition
when a run stops for a reason this module cannot carry further.  A loop
made without EMIT builds an emitter of its own, so a story can watch a
run without wiring a console."
  (pearl-loop--create
   :provider provider
   :tools tools
   :session session
   :emit (cond ((null emit) (pearl-event-emitter))
               ((functionp emit) (pearl-loop--subscribers emit))
               (t emit))
   :guard guard
   :on-error (or on-error #'ignore)))

(defun pearl-loop--subscribers (dispatcher)
  "An emitter that hands every event to DISPATCHER.

A story watches a run by passing a function where an emitter goes, and a
function cannot be type-checked the way an emitter can, so it is wrapped
once here instead of being tested for at every emit."
  (let ((emitter (pearl-event-emitter)))
    (pearl-event-subscribe emitter dispatcher)
    emitter))

(defun pearl-loop-emitter (loop)
  "The emitter LOOP speaks through."
  (pearl-loop-emit loop))

(defun pearl-loop-busy-p (loop)
  "Non-nil while LOOP has a run in flight."
  (eq (pearl-loop-state loop) 'running))

(defun pearl-loop-queued (loop &optional kind)
  "The messages waiting in LOOP, oldest first.

KIND is `steering', `follow-up', or nil for both in delivery order."
  (pcase kind
    ('steering (reverse (pearl-loop-steering loop)))
    ('follow-up (reverse (pearl-loop-follow-ups loop)))
    (_ (append (reverse (pearl-loop-steering loop))
               (reverse (pearl-loop-follow-ups loop))))))

(defun pearl-loop--emit (loop kind data)
  "Emit KIND carrying DATA on LOOP's emitter."
  (pearl-event-emit (pearl-loop-emit loop) (pearl-event-new kind data)))

(defun pearl-loop--alive-p (loop)
  "Non-nil when LOOP's owner is still there."
  (let ((guard (pearl-loop-guard loop)))
    (or (null guard) (and (funcall guard) t))))

(defun pearl-loop--dispatch (loop step)
  "Run STEP for LOOP on a zero-delay timer when this run is still current.

Every wait in this module goes through here, which is where ADR 10's
three rules live: defer, check the generation, and report a dead owner
instead of dropping the chain."
  (let ((generation (pearl-loop-generation loop)))
    (run-at-time 0 nil
                 (lambda ()
                   (when (and (eq generation (pearl-loop-generation loop))
                              (pearl-loop-busy-p loop))
                     (if (pearl-loop--alive-p loop)
                         (condition-case error
                             (funcall step)
                           (error (pearl-loop--fail loop error)))
                       (pearl-loop--dead loop)))))))

(defun pearl-loop--stop (loop)
  "Make LOOP idle and stale, and return the handle it was waiting on."
  (let ((handle (pearl-loop-handle loop)))
    (setf (pearl-loop-generation loop) (1+ (pearl-loop-generation loop))
          (pearl-loop-state loop) 'idle
          (pearl-loop-handle loop) nil
          (pearl-loop-turn loop) nil)
    handle))

(defun pearl-loop--dead (loop)
  "Stop LOOP because its owner is gone, and say so once."
  (pearl-loop--stop loop)
  (funcall (pearl-loop-on-error loop) (list 'pearl-loop-dead-owner)))

(defun pearl-loop--fail (loop condition)
  "Stop LOOP after CONDITION, reporting it through the terminal callback."
  (let ((handle (pearl-loop--stop loop)))
    (when (and handle (pearl-loop-provider loop))
      (condition-case nil
          (pearl-provider-cancel (pearl-loop-provider loop) handle)
        (error nil))))
  (funcall (pearl-loop-on-error loop) condition))

(defun pearl-loop-abort (loop)
  "Stop LOOP, keep what the tools already produced, and return the queues.

The queues come back in delivery order: a caller that gets them back is
going to retype them or drop them, and either way the order is the
caller's, not this module's (ADR 8, pi's abort)."
  (let ((handed (pearl-loop-queued loop))
        (handle nil))
    (setf (pearl-loop-steering loop) nil
          (pearl-loop-follow-ups loop) nil)
    (setq handle (pearl-loop--stop loop))
    (when (and handle (pearl-loop-provider loop))
      (condition-case nil
          (pearl-provider-cancel (pearl-loop-provider loop) handle)
        (error nil)))
    handed))

(defun pearl-loop-steer (loop message)
  "Queue MESSAGE for LOOP, to be delivered after the turn's tool results."
  (setf (pearl-loop-steering loop) (cons message (pearl-loop-steering loop)))
  nil)

(defun pearl-loop-follow-up (loop message)
  "Queue MESSAGE for LOOP, to be delivered when it would otherwise stop."
  (setf (pearl-loop-follow-ups loop) (cons message (pearl-loop-follow-ups loop)))
  nil)

(defun pearl-loop-set-tools (loop tools)
  "Replace the tool list of LOOP with TOOLS, declaring the difference.

The declaration is a system message on the record and not a flag in
memory, so replaying the transcript yields the same tool set (ADR 8)."
  (let* ((before (mapcar (lambda (tool) (plist-get tool :name))
                         (pearl-loop-tools loop)))
         (after (mapcar (lambda (tool) (plist-get tool :name)) tools)))
    (setf (pearl-loop-tools loop) tools)
    (unless (and (equal before after) t)
      (pearl-loop--record-message
       loop
       (pearl-contract-message
        'system
        (format "Tools are now: %s. Added: %s. Removed: %s."
                (if after (mapconcat #'identity after ", ") "none")
                (let ((added (seq-difference after before)))
                  (if added (mapconcat #'identity added ", ") "none"))
                (let ((removed (seq-difference before after)))
                  (if removed (mapconcat #'identity removed ", ") "none"))))))
    nil))

(defun pearl-loop--record-message (loop message)
  "Record MESSAGE on LOOP's session and return the entry."
  (pearl-session-append
   (pearl-loop-session loop)
   (pearl-contract-entry 'message
                         (list :role (plist-get message :role)
                               :content (pearl-contract-text message)
                               :timestamp (plist-get message :timestamp)))))

(defun pearl-loop--record-result (loop call result)
  "Append RESULT, the answer to CALL, to the session of LOOP."
  (pearl-session-append
   (pearl-loop-session loop)
   (pearl-contract-entry 'tool
                         (list :id (plist-get call :id)
                               :name (plist-get call :name)
                               :content (plist-get result :content)
                               :error (plist-get result :error))
                         nil (plist-get result :patch))))

(defun pearl-loop-submit (loop prompt)
  "Admit PROMPT to LOOP and start a run; return the run id.

The run happens on the event loop and not inside this call, so the
editor never waits for a model (ADR 10)."
  (when (pearl-loop-busy-p loop)
    (signal 'pearl-loop-busy nil))
  (setf (pearl-loop-state loop) 'running)
  (setf (pearl-loop-generation loop) (1+ (pearl-loop-generation loop)))
  (setf (pearl-loop-run loop) (pearl-loop-generation loop))
  (let ((id (pearl-loop-run loop)))
    (pearl-loop--dispatch
     loop
     (lambda ()
       (pearl-loop--record-message loop prompt)
       (pearl-loop--emit loop 'agent-start (list :run id))
       (pearl-loop--turn loop)))
    id))

(defun pearl-loop--turn (loop)
  "Ask the provider of LOOP for one response and run the tool call it made."
  (setf (pearl-loop-turn loop) (pearl-turn-new)
        (pearl-loop-finished loop) nil
        (pearl-loop-started loop) nil)
  (let* ((turn (pearl-loop-turn loop))
         (declarations (pearl-tool-declarations (pearl-loop-tools loop)))
         (request (pearl-contract-request
                   (pearl-session-projection (pearl-loop-session loop) declarations)
                   :tools declarations)))
    (pearl-loop--emit loop 'turn-start (list :run (pearl-loop-run loop)))
    (setf (pearl-loop-handle loop)
          (pearl-provider-request
           (pearl-loop-provider loop) request
           (lambda (event) (pearl-loop--event loop turn event))
           :on-done (lambda () (pearl-loop--response-done loop turn))
           :on-error (lambda (condition) (pearl-loop--fail loop condition))))))

(defun pearl-loop--event (loop turn event)
  "Take one EVENT from the provider of LOOP into TURN."
  (pearl-turn-feed turn event)
  (pcase (plist-get event :type)
    ('text
     (unless (pearl-loop-started loop)
       (setf (pearl-loop-started loop) t)
       (pearl-loop--emit loop 'message-start
                         (list :message (pearl-contract-message 'assistant ""))))
     (pearl-loop--emit loop 'message-update
                       (list :message (or (pearl-turn-message turn)
                                          (pearl-contract-message 'assistant ""))
                             :delta (plist-get event :text))))
    ('stop (pearl-loop--finish-turn loop turn))))

(defun pearl-loop--response-done (loop turn)
  "Finish TURN once the provider of LOOP has stopped talking."
  (unless (pearl-turn-stop turn)
    (pearl-turn-feed turn (list :type 'stop :reason 'end)))
  (pearl-loop--finish-turn loop turn))

(defun pearl-loop--finish-turn (loop turn)
  "Record what TURN produced for LOOP, then continue or stop."
  (unless (pearl-loop-finished loop)
    (setf (pearl-loop-finished loop) t)
    (let* ((message (pearl-turn-message turn))
           (calls (pearl-turn-calls turn))
           (truncated (eq (pearl-turn-stop turn) 'length)))
      (when (and message (pearl-loop-started loop))
        (pearl-loop--emit loop 'message-end (list :message message))
        (pearl-loop--record-message loop message))
      (when calls (pearl-loop--run-calls loop turn))
      (pearl-loop--drain-steering loop)
      (cond
       ((and calls (not truncated))
        (pearl-loop--dispatch loop (lambda () (pearl-loop--turn loop))))
       (t
        (pearl-loop--emit loop 'turn-end (list :run (pearl-loop-run loop)))
        (pearl-loop--dispatch loop (lambda () (pearl-loop--continue loop))))))))

(defun pearl-loop--run-calls (loop turn)
  "Run every tool call TURN made for LOOP, in order, recording each answer."
  (let ((truncated (eq (pearl-turn-stop turn) 'length)))
    (dolist (call (pearl-turn-calls turn))
      (let* ((name (plist-get call :name))
             (id (plist-get call :id))
             (arguments (plist-get call :arguments))
             (tool (seq-find (lambda (candidate)
                               (equal name (plist-get candidate :name)))
                             (pearl-loop-tools loop))))
        (pearl-loop--emit loop 'tool-execution-start
                          (list :run (pearl-loop-run loop) :id id :name name))
        (let ((result
               (cond
                (truncated
                 (pearl-tool-error-result call "the response was truncated"))
                ((null tool)
                 (pearl-tool-error-result call (format "no tool named %s" name)))
                ((not (listp arguments))
                 (pearl-tool-error-result call "arguments are not JSON"))
                (t (pearl-tool-run tool arguments (list :loop loop))))))
          (pearl-loop--emit loop 'tool-execution-end
                            (list :run (pearl-loop-run loop) :id id :result result))
          (pearl-loop--record-result loop call result))))))

(defun pearl-loop--deliver (loop message)
  "Put MESSAGE on LOOP's record, saying so on the way in.

A queued message becomes part of the transcript when it is delivered and
not when it was typed, so a console that only watches events still shows
it in the right place."
  (pearl-loop--emit loop 'message-start (list :message message))
  (pearl-loop--record-message loop message)
  (pearl-loop--emit loop 'message-end (list :message message)))

(defun pearl-loop--drain-steering (loop)
  "Put LOOP's steering queue on the record, oldest first.

Steering lands after the turn's tool results and before the next
request, which is the whole of what makes it steering and not a follow
up (ADR 8): the model has read what its own calls produced, and has not
yet stopped."
  (let ((queued (reverse (pearl-loop-steering loop))))
    (when queued
      (setf (pearl-loop-steering loop) nil)
      (dolist (message queued) (pearl-loop--deliver loop message)))
    queued))

(defun pearl-loop--continue (loop)
  "Deliver what LOOP has queued, or hand the run back to the editor."
  (let ((steering (pearl-loop--drain-steering loop))
        (follow-ups (reverse (pearl-loop-follow-ups loop))))
    (cond
     (steering (pearl-loop--turn loop))
     (follow-ups
      (setf (pearl-loop-follow-ups loop) nil)
      (dolist (message follow-ups) (pearl-loop--deliver loop message))
      (pearl-loop--turn loop))
     (t
      (pearl-loop--emit loop 'agent-end (list :run (pearl-loop-run loop)))
      (pearl-loop--stop loop)
      nil))))

(provide 'pearl-loop)

;;; pearl-loop.el ends here
