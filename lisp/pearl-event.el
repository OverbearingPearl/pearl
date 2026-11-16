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
;; TODO `pearl-event-subscribe' (DISPATCHER) -> unsubscribe function
;;      DISPATCHER is called with one event.  Subscriptions are ordered and
;;      kept per emitter, not globally, so two sessions never see each other.
;;
;; TODO `pearl-event-emit' (EMITTER EVENT) -> nil
;;      Calls every subscriber in order.  A subscriber that signals is reported
;;      to the error callback of the run and does not stop the emission.
;;      Borrow: pi never awaits an emitter; the same rule here.
;;
;; TODO `pearl-event-render-hint' (EVENT) -> symbol or nil
;;      What the console should do with EVENT: `append', `replace-last',
;;      `status', or nil for events a surface shows indirectly.  Keeps the
;;      vocabulary of rendering out of the loop.

;;; Code:

(provide 'pearl-event)

;;; pearl-event.el ends here
