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

(ert-gwt-deftest
  (:given ((event nil)))
  (:when (setq event (pearl-event-new 'message-update '(:message "x" :delta "y"))))
  (:then (eq 'append (pearl-event-render-hint event))))

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
  (:given ((emitter (pearl-event-emitter))))
  (:when (pearl-event-new 'made-up '(:run "r1")))
  (:then (should-error (pearl-event-new 'made-up '(:run "r1")) :type 'pearl-event-invalid)))

;; An unknown key in the data is an error too: a typo in a payload must not
;; become a field nobody reads.
(ert-gwt-deftest
  (:given ((event nil)))
  (:when (setq event (pearl-event-new 'message-update '(:delta "y" :bogus 1))))
  (:then (should-error (pearl-event-new 'message-update '(:delta "y" :bogus 1)))))

;; Subscribing returns the way out: after the unsubscribe runs, the
;; listener hears nothing.
(ert-gwt-deftest
  (:given ((seen nil) (emitter (pearl-event-emitter)) (unsubscribe nil)))
  (:when (progn (setq unsubscribe (pearl-event-subscribe emitter (lambda (event) (push event seen)))) (funcall unsubscribe) (pearl-event-emit emitter (pearl-event-new 'turn-end '(:run "r1")))))
  (:then (null seen)))

;; Two emitters are two sessions: what one says, the other never hears.
(ert-gwt-deftest
  (:given ((seen nil) (one (pearl-event-emitter)) (two (pearl-event-emitter))))
  (:when (progn (pearl-event-subscribe two (lambda (event) (push event seen))) (pearl-event-emit one (pearl-event-new 'turn-end '(:run "r1")))))
  (:then (null seen)))

;; Render hints keep the rendering vocabulary out of the loop: a tool
;; starting is status, an agent starting is shown indirectly.
(ert-gwt-deftest
  (:given ((tool-start (pearl-event-new 'tool-execution-start '(:run "r1" :id "c1" :name "read")))
           (agent-start (pearl-event-new 'agent-start '(:run "r1")))))
  (:when (list (pearl-event-render-hint tool-start) (pearl-event-render-hint agent-start)))
  (:then (eq 'status (pearl-event-render-hint tool-start)))
  (:then (null (pearl-event-render-hint agent-start))))

(provide 'event-tests)

;;; event-tests.el ends here
