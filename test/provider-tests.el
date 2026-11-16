;;; provider-tests.el --- stories about the request interface  -*- lexical-binding: t; -*-

;;; Commentary:

;; A provider answers with events, and the fake is the interface used
;; twice: a whole turn becomes data a test can assert on.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake (quote ((text "hello") (stop "end"))))) (seen nil)))
  (:when (pearl-provider-request provider nil (lambda (event) (push event seen)) :on-done (lambda () nil) :on-error (function ignore)))
  (:then (equal "hello" (plist-get (car (last seen)) :text))))

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake (quote ((delay 0.1) (text "late") (stop "end"))))) (seen nil) (handle nil)))
  (:when (progn (setq handle (pearl-provider-request provider nil (lambda (event) (push event seen)) :on-done (lambda () nil) :on-error (function ignore))) (pearl-provider-cancel provider handle) (sleep-for 0.2)))
  (:then (null (seq-find (lambda (event) (equal "late" (plist-get event :text))) seen))))

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake (quote ((stop "end"))) :fail-with (quote error))) (failure nil)))
  (:when (pearl-provider-request provider nil (function ignore) :on-done (lambda () nil) :on-error (lambda (err) (setq failure err))))
  (:then failure))
(provide 'provider-tests)

;;; provider-tests.el ends here
