;;; provider-tests.el --- stories about the request interface  -*- lexical-binding: t; -*-

;;; Commentary:

;; A provider answers with events, and the fake is the interface used
;; twice: a whole turn becomes data a test can assert on.

;;; Code:

(require 'support-tests)
(require 'pearl-provider)
(require 'ert-gwt)

;; A text turn arrives as events: the caller sees the text the model wrote,
;; and the request ends when the fake says it does.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "hello") (stop "end")))) (seen nil) (done 0)))
  (:when (progn (pearl-provider-request provider nil (lambda (event) (push event seen)) :on-done (lambda () (setq done (1+ done))) :on-error #'ignore) (support-tests-wait (lambda () (= 1 done)))))
  (:then (equal "hello" (plist-get (car (last seen)) :text)))
  (:then (equal 2 (length seen))))

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((delay 0.1) (text "late") (stop "end")))) (seen nil) (handle nil)))
  (:when (progn (setq handle (pearl-provider-request provider nil (lambda (event) (push event seen)) :on-done (lambda () nil) :on-error #'ignore)) (pearl-provider-cancel provider handle) (sleep-for 0.2)))
  (:then (null (seq-find (lambda (event) (equal "late" (plist-get event :text))) seen))))

;; Failure reaches the caller as the condition that failed, never as a
;; message string it would have to parse back into a symbol.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((stop "end")) :fail-with 'error)) (failure nil)))
  (:when (progn (pearl-provider-request provider nil #'ignore :on-done (lambda () nil) :on-error (lambda (err) (setq failure err)))
                (support-tests-wait (lambda () failure))))
  (:then (consp failure))
  (:then (eq 'error (car failure))))

;; A whole turn as data: text, a tool call with its arguments, and the stop
;; reason, in script order, with the done callback run exactly once.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "hello") (tool-call "c1" "read" ((path . "a.el"))) (stop "end"))))
           (seen nil) (done 0)))
  (:when (progn (pearl-provider-request provider nil (lambda (event) (push event seen)) :on-done (lambda () (setq done (1+ done))) :on-error #'ignore) (support-tests-wait (lambda () (= 1 done)))))
  (:then (equal 1 done))
  (:then (equal '("hello" "c1" "end") (mapcar (lambda (event) (or (plist-get event :text) (plist-get event :id) (plist-get event :reason))) (reverse seen))))
  (:then (equal "read" (plist-get (cadr seen) :name)))
  (:then (equal "a.el" (alist-get 'path (plist-get (cadr seen) :arguments)))))

;; Done runs once either way: a failed request still releases the caller.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((stop "end")) :fail-with 'error)) (done 0) (failure nil)))
  (:when (progn (pearl-provider-request provider nil #'ignore :on-done (lambda () (setq done (1+ done))) :on-error (lambda (err) (setq failure err))) (support-tests-wait (lambda () (= 1 done)))))
  (:then (equal 1 done))
  (:then (consp failure))
  (:then (eq 'error (car failure))))

;; Cancel is idempotent, and a request cancelled twice before its first
;; step still delivers nothing: the second cancel is not an error and not a
;; second chance for the reply to arrive.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((delay 0.05) (text "never") (stop "end")))) (seen nil) (handle nil)))
  (:when (progn (setq handle (pearl-provider-request provider nil (lambda (event) (push event seen)) :on-done #'ignore :on-error #'ignore)) (pearl-provider-cancel provider handle) (pearl-provider-cancel provider handle) (sleep-for 0.2)))
  (:then (null seen)))

;; The fake records the requests it was asked: a story can assert on what
;; the model would have been told, not only on what it answered.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "one") (stop "end")))) (request '(:messages ((:role "user" :content "go")) :model "m"))))
  (:when (pearl-provider-request provider request #'ignore :on-done #'ignore :on-error #'ignore))
  (:then (equal (list request) (pearl-provider-fake-requests provider))))

;; The interface is a predicate: the fake satisfies it, an arbitrary
;; object does not, so a caller can swap one for the other in one place.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((stop "end")))) (accepted nil)))
  (:when (setq accepted (pearl-provider-p provider)))
  (:then accepted)
  (:then (not (pearl-provider-p 'not-a-provider))))

(provide 'provider-tests)

;;; provider-tests.el ends here
