;;; pearl-event.el --- the event kinds and their listeners  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; What the loop says as it runs.  Events go out and nothing comes back along
;; that edge: the loop does not wait for a listener, and a listener that fails
;; is reported rather than allowed to stop a run.
;;
;; The order of the kinds is part of the contract, not an implementation
;; detail, because a console that renders them out of order draws a
;; conversation that never happened.  ADR 8 pins the order to Pi's.
;;
;; An event carries data and no instruction about how to show it.  There used
;; to be a render hint here - `append', `replace-last', `status' - and it was
;; the core writing a console's vocabulary, which is the one thing ADR 26 says
;; the core does not do.  A surface switches on the kind.
;;
;; Planned API
;;
;; `pearl-event-kinds' -> constant list of symbols
;;      `agent-start', `turn-start', `message-start', `message-update',
;;      `message-end', `tool-execution-start', `tool-execution-update',
;;      `tool-execution-end', `turn-end', `agent-end'.  Borrow: pi's event
;;      taxonomy, pinned in ADR 8.
;;
;; `pearl-event-new' (KIND DATA) -> event
;;      Signals `pearl-event-invalid' for a kind outside the list.  DATA is a
;;      plist whose required keys depend on KIND; an unknown key is an error.
;;
;; `pearl-event-emitter' () -> emitter
;;      An emitter is what the loop is given as its `:emit' and what a
;;      surface subscribes to.  It is an object rather than a global list, so
;;      two sessions in one Emacs never see each other's events.
;;
;; `pearl-event-subscribe' (EMITTER DISPATCHER) -> unsubscribe function
;;      DISPATCHER is called with one event.  Subscriptions are ordered and
;;      kept per emitter, not globally, so two sessions never see each other.
;;
;; `pearl-event-emit' (EMITTER EVENT) -> nil
;;      Calls every subscriber in order.  A subscriber that signals is reported
;;      to the error callback of the run and does not stop the emission.
;;      Borrow: pi never awaits an emitter; the same rule here.

;;; Code:

(require 'cl-lib)
(require 'subr-x)

(define-error 'pearl-event-invalid "An event is not one the taxonomy knows")

(defconst pearl-event--kinds
  '(agent-start turn-start message-start message-update message-end
    tool-execution-start tool-execution-update tool-execution-end
    turn-end agent-end)
  "The ten kinds, in the order a run emits them (ADR 8).")

(defun pearl-event-kinds ()
  "The kinds a run emits, in order."
  pearl-event--kinds)

(defconst pearl-event-keys
  '((agent-start :run)
    (turn-start :run)
    (message-start :message)
    (message-update :message :delta)
    (message-end :message)
    (tool-execution-start :run :id :name)
    (tool-execution-update :run :id :delta)
    (tool-execution-end :run :id :result)
    (turn-end :run)
    (agent-end :run))
  "The keys each kind carries.

A key this table names for one kind is a key the taxonomy knows how to read,
so it is allowed anywhere; a key it names nowhere is a typo.  Which kinds
require which of them is the value, and `pearl-event-new' checks both.

A message event does not repeat the run: it names the message it is about,
and the emitter it arrives on is already one session's.")

(cl-defstruct (pearl-event-emitter (:constructor pearl-event-emitter--create))
  listeners)

(defun pearl-event-emitter ()
  "An emitter, owned by whoever asked for it."
  (pearl-event-emitter--create))

(defun pearl-event-subscribe (emitter dispatcher)
  "Call DISPATCHER with every event EMITTER carries; return the way out."
  (push dispatcher (pearl-event-emitter-listeners emitter))
  (lambda ()
    (setf (pearl-event-emitter-listeners emitter)
          (delq dispatcher (pearl-event-emitter-listeners emitter)))))

(defun pearl-event-emit (emitter event)
  "Hand EVENT to every subscriber of EMITTER, in order.

A subscriber that signals is dropped and not propagated: nothing waits on this
edge, and a run does not stop because a surface failed to draw one line.  The
subscriber's own surface reports its own failure."
  (dolist (dispatcher (copy-sequence (pearl-event-emitter-listeners emitter)))
    (condition-case nil
        (funcall dispatcher event)
      (error nil))))

(defun pearl-event--key-name (key)
  "KEY without the colon a plist key carries, as a symbol."
  (intern (string-remove-prefix ":" (symbol-name key))))

(defun pearl-event-new (kind data)
  "An event of KIND carrying DATA."
  (unless (memq kind pearl-event--kinds)
    (signal 'pearl-event-invalid (list :kind kind)))
  (let ((known (apply #'append (mapcar #'cdr pearl-event-keys))))
    (cl-loop for (key _value) on data by #'cddr
             unless (memq key known)
             do (signal 'pearl-event-invalid
                       (list :key (pearl-event--key-name key) :kind kind))))
  (dolist (key (alist-get kind pearl-event-keys))
    (unless (plist-member data key)
      (signal 'pearl-event-invalid
                (list :key (pearl-event--key-name key) :kind kind :missing t))))
  (list :kind kind :data data))

(provide 'pearl-event)

;;; pearl-event.el ends here
