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
;; TODO `pearl-event-kinds' -> constant list of symbols
;;      `agent-start', `turn-start', `message-start', `message-update',
;;      `message-end', `tool-execution-start', `tool-execution-update',
;;      `tool-execution-end', `turn-end', `agent-end'.  Borrow: pi's event
;;      taxonomy, pinned in ADR 8.
;;
;; TODO `pearl-event-new' (KIND DATA) -> event
;;      Signals `pearl-event-invalid' for a kind outside the list.  DATA is a
;;      plist whose required keys depend on KIND; an unknown key is an error.
;;
;; TODO `pearl-event-emitter' () -> emitter
;;      An emitter is what the loop is given as its `:emit' and what a
;;      surface subscribes to.  It is an object rather than a global list, so
;;      two sessions in one Emacs never see each other's events.
;;
;; TODO `pearl-event-subscribe' (EMITTER DISPATCHER) -> unsubscribe function
;;      DISPATCHER is called with one event.  Subscriptions are ordered and
;;      kept per emitter, not globally, so two sessions never see each other.
;;
;; TODO `pearl-event-emit' (EMITTER EVENT) -> nil
;;      Calls every subscriber in order.  A subscriber that signals is reported
;;      to the error callback of the run and does not stop the emission.
;;      Borrow: pi never awaits an emitter; the same rule here.

;;; Code:

(provide 'pearl-event)

;;; pearl-event.el ends here
