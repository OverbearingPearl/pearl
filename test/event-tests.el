;;; event-tests.el --- stories about what the loop says  -*- lexical-binding: t; -*-

;;; Commentary:

;; A console draws the conversation out of these events, so their order is
;; the conversation: a listener sees what happened, in that order.

;;; Code:

(require 'support-tests)
(require 'pearl-event)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((seen nil) (emitter (pearl-event-emitter)) (start (pearl-event-new 'agent-start '(:run "r1"))) (turn (pearl-event-new 'turn-start '(:run "r1")))))
  (:when (progn (pearl-event-subscribe emitter (lambda (event) (push (plist-get event :kind) seen))) (pearl-event-emit emitter start) (pearl-event-emit emitter turn)))
  (:then (equal '(turn-start agent-start) seen)))

(ert-gwt-deftest
  (:given ((seen nil) (emitter (pearl-event-emitter)) (event (pearl-event-new 'turn-end '(:run "r1")))))
  (:when (progn (pearl-event-subscribe emitter (lambda (_event) (error "A listener failed"))) (pearl-event-subscribe emitter (lambda (event) (push (plist-get event :kind) seen))) (pearl-event-emit emitter event)))
  (:then (equal '(turn-end) seen)))

;; The taxonomy is the contract, pinned to Pi's by ADR 8: exactly these
;; kinds, in exactly this order.
(ert-gwt-deftest
  (:given ((kinds nil)))
  (:when (setq kinds (pearl-event-kinds)))
  (:then (equal '(agent-start turn-start message-start message-update
                  message-end tool-execution-start tool-execution-update
                  tool-execution-end turn-end agent-end)
                kinds)))

;; A kind outside the list is an error at construction, not an event a
;; console would silently drop.
(ert-gwt-deftest
  (:given ((err nil)))
  (:when (condition-case e (pearl-event-new 'made-up '(:run "r1")) (error (setq err e))))
  (:then (eq 'pearl-event-invalid (car err)))
  (:then (equal 'made-up (plist-get (cdr err) :kind))))

;; An unknown key in the data is an error too: a typo in a payload must not
;; become a field nobody reads.
(ert-gwt-deftest
  (:given ((err nil)))
  (:when (condition-case e (pearl-event-new 'message-update '(:delta "y" :bogus 1))
           (error (setq err e))))
  (:then (eq 'pearl-event-invalid (car err)))
  (:then (equal 'bogus (plist-get (cdr err) :key))))

;; Subscribing returns the way out: the listener hears what was emitted
;; before the unsubscribe ran, and nothing after it.
(ert-gwt-deftest
  (:given ((seen nil) (emitter (pearl-event-emitter)) (unsubscribe nil)
           (first (pearl-event-new 'turn-start '(:run "r1")))
           (second (pearl-event-new 'turn-end '(:run "r1")))))
  (:when (progn (setq unsubscribe (pearl-event-subscribe emitter (lambda (event) (push (plist-get event :kind) seen))))
                (pearl-event-emit emitter first)
                (funcall unsubscribe)
                (pearl-event-emit emitter second)))
  (:then (equal '(turn-start) seen)))

;; Two emitters are two sessions: each listener hears its own emitter and
;; nothing the other one said.
(ert-gwt-deftest
  (:given ((seen nil) (one (pearl-event-emitter)) (two (pearl-event-emitter))
           (first (pearl-event-new 'turn-start '(:run "one")))
           (second (pearl-event-new 'turn-end '(:run "two")))))
  (:when (progn (pearl-event-subscribe two (lambda (event) (push (plist-get event :kind) seen)))
                (pearl-event-emit one first)
                (pearl-event-emit two second)))
  (:then (equal '(turn-end) seen)))

(provide 'event-tests)

;;; event-tests.el ends here
