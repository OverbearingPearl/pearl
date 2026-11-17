;;; provider-tests.el --- stories about the request interface  -*- lexical-binding: t; -*-

;;; Commentary:

;; A provider answers with events, and the fake is the interface used
;; twice: a whole turn becomes data a test can assert on.

;;; Code:

(require 'support-tests)
(require 'pearl-provider)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "hello") (stop "end")))) (seen nil)))
  (:when (pearl-provider-request provider nil (lambda (event) (push event seen)) :on-done (lambda () nil) :on-error #'ignore))
  (:then (equal "hello" (plist-get (car (last seen)) :text))))

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((delay 0.1) (text "late") (stop "end")))) (seen nil) (handle nil)))
  (:when (progn (setq handle (pearl-provider-request provider nil (lambda (event) (push event seen)) :on-done (lambda () nil) :on-error #'ignore)) (pearl-provider-cancel provider handle) (sleep-for 0.2)))
  (:then (null (seq-find (lambda (event) (equal "late" (plist-get event :text))) seen))))

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((stop "end")) :fail-with 'error)) (failure nil)))
  (:when (pearl-provider-request provider nil #'ignore :on-done (lambda () nil) :on-error (lambda (err) (setq failure err))))
  (:then failure)
  (:then (consp failure))
  (:then (not (stringp failure))))

;; A whole turn as data: text, a tool call with its arguments, and the stop
;; reason, in script order, with the done callback run exactly once.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "hello") (tool-call "c1" "read" ((path . "a.el"))) (stop "end"))))
           (seen nil) (done 0)))
  (:when (progn (pearl-provider-request provider nil (lambda (event) (push event seen)) :on-done (lambda () (setq done (1+ done))) :on-error #'ignore) (support-tests-wait (lambda () (= 1 done)))))
  (:then (equal 1 done))
  (:then (equal '("hello" "c1" "end") (mapcar (lambda (event) (or (plist-get event :text) (plist-get event :id) (plist-get event :reason))) (nreverse seen))))
  (:then (equal "read" (plist-get (cadr seen) :name)))
  (:then (equal "a.el" (alist-get 'path (plist-get (cadr seen) :arguments)))))

;; Done runs once either way: a failed request still releases the caller.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((stop "end")) :fail-with 'error)) (done 0) (failure nil)))
  (:when (progn (pearl-provider-request provider nil #'ignore :on-done (lambda () (setq done (1+ done))) :on-error (lambda (err) (setq failure err))) (support-tests-wait (lambda () (= 1 done)))))
  (:then (equal 1 done))
  (:then failure))

;; Cancel is idempotent, and a hanging request that is cancelled never
;; delivers anything afterwards.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake '((text "never")) :hang t)) (seen nil) (handle nil)))
  (:when (progn (setq handle (pearl-provider-request provider nil (lambda (event) (push event seen)) :on-done #'ignore :on-error #'ignore)) (pearl-provider-cancel provider handle) (pearl-provider-cancel provider handle) (sleep-for 0.1)))
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
  (:given ((provider (pearl-provider-fake '((stop "end"))))))
  (:when (list (pearl-provider-p provider) (pearl-provider-p 'not-a-provider)))
  (:then (pearl-provider-p provider))
  (:then (not (pearl-provider-p 'not-a-provider))))

(provide 'provider-tests)

;;; provider-tests.el ends here
