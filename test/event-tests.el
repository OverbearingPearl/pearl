;;; event-tests.el --- stories about what the loop says  -*- lexical-binding: t; -*-

;;; Commentary:

;; A console draws the conversation out of these events, so their order is
;; the conversation: a listener sees what happened, in that order.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((seen nil) (emitter (make-hash-table :test (function eq))) (start (pearl-event-new 'agent-start '(:run "r1"))) (turn (pearl-event-new 'turn-start '(:run "r1")))))
  (:when (progn (pearl-event-subscribe emitter (lambda (event) (push (plist-get event :kind) seen))) (pearl-event-emit emitter start) (pearl-event-emit emitter turn)))
  (:then (equal (quote (turn-start agent-start)) seen)))

(ert-gwt-deftest
  (:given ((seen nil) (emitter (make-hash-table :test (function eq))) (event (pearl-event-new 'turn-end '(:run "r1")))))
  (:when (progn (pearl-event-subscribe emitter (lambda (_event) (error "A listener failed"))) (pearl-event-subscribe emitter (lambda (event) (push (plist-get event :kind) seen))) (pearl-event-emit emitter event)))
  (:then (equal (quote (turn-end)) seen)))

(ert-gwt-deftest
  (:given ((event nil)))
  (:when (setq event (pearl-event-new 'message-update '(:message "x" :delta "y"))))
  (:then (eq (quote append) (pearl-event-render-hint event))))
(provide 'event-tests)

;;; event-tests.el ends here
