;;; loop-tests.el --- stories about the double loop  -*- lexical-binding: t; -*-

;;; Commentary:

;; The two channels are the whole control flow: steering arrives mid-run
;; and never ends it, follow-up arrives only when the loop would stop.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake (quote ((text "one") (stop "end"))))) (emit (lambda (_event) nil)) (loop (pearl-loop-new :provider provider :tools nil :session (pearl-session-new) :emit emit))))
  (:when (progn (pearl-loop-steer loop (pearl-contract-message (quote user) "stop here")) (pearl-loop-submit loop (pearl-contract-message (quote user) "go"))))
  (:then (equal "stop here" (plist-get (pearl-loop-queued loop (quote steering)) :content))))

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake (quote ((text "one") (stop "end"))))) (loop (pearl-loop-new :provider provider :tools nil :session (pearl-session-new) :emit (function ignore)))))
  (:when (pearl-loop-submit loop (pearl-contract-message (quote user) "go")))
  (:then (null (pearl-loop-queued loop (quote follow-up))))
  (:then (null (pearl-loop-busy-p loop))))

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake (quote ((text "one") (stop "end"))))) (loop (pearl-loop-new :provider provider :tools nil :session (pearl-session-new) :emit (function ignore))) (handed nil)))
  (:when (progn (pearl-loop-submit loop (pearl-contract-message (quote user) "go")) (setq handed (pearl-loop-abort loop))))
  (:then (listp handed))
  (:then (null (pearl-loop-busy-p loop))))

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake (quote ((text "one") (stop "end"))))) (tool (pearl-tool-define "read" (quote (:description "read a file")) (lambda (_arguments _context) "read"))) (results nil) (loop (pearl-loop-new :provider provider :tools (list tool) :session (pearl-session-new) :emit (lambda (event) (when (eq (plist-get event :kind) (quote tool-execution-end)) (push event results)))))))
  (:when (pearl-loop-submit loop (pearl-contract-message (quote user) "go")))
  (:then (null results)))

(ert-gwt-deftest
  (:given ((provider (pearl-provider-fake (quote ((text "cut") (stop "length"))))) (tool (pearl-tool-define "read" (quote (:description "read")) (lambda (_arguments _context) "read"))) (loop (pearl-loop-new :provider provider :tools (list tool) :session (pearl-session-new) :emit (function ignore)))))
  (:when (pearl-loop-submit loop (pearl-contract-message (quote user) "go")))
  (:then (null (pearl-loop-queued loop (quote steering)))))
(provide 'loop-tests)

;;; loop-tests.el ends here
