;;; tools-shell-tests.el --- stories about a command  -*- lexical-binding: t; -*-

;;; Commentary:

;; A command's effects cannot be taken back, so this is the one tool that
;; asks, and it asks only where it cannot prove the write scope.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((context (list :root default-directory)) (result nil)))
  (:when (setq result (pearl-tools-shell-run (list (cons (quote command) "echo hi") (cons (quote reason) "show the greeting")) context)))
  (:then (equal 0 (plist-get result :status)))
  (:then (equal "hi\n" (plist-get result :stdout))))

(ert-gwt-deftest
  (:given ((command "curl -s https://example.com -o /tmp/x") (scope nil)))
  (:when (pearl-tools-shell-confirm command scope "download a page"))
  (:then (not (pearl-tools-shell-confirm command scope "download a page"))))

(ert-gwt-deftest
  (:given ((scope nil)))
  (:when (pearl-tools-shell-scope "git push origin main" default-directory))
  (:then (null (pearl-tools-shell-scope "git push origin main" default-directory))))
(provide 'tools-shell-tests)

;;; tools-shell-tests.el ends here
