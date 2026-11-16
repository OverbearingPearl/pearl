;;; tool-tests.el --- stories about the trust boundary  -*- lexical-binding: t; -*-

;;; Commentary:

;; A tool is where model text becomes an action, so this is where
;; arguments are checked once and where every failure stays a result.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((tool (pearl-tool-define "boom" (quote (:description "always fails")) (lambda (_arguments _context) (error "No such file")))) (result nil)))
  (:when (setq result (pearl-tool-run tool nil nil)))
  (:then (plist-get result :error)))

(ert-gwt-deftest
  (:given ((tool (pearl-tool-define "read" (quote (:description "read" :arguments ((path :required t)))) (lambda (_arguments _context) "read"))) (arguments '((offset . 3)))))
  (:when (pearl-tool-validate tool arguments))
  (:then (should-error (pearl-tool-validate tool arguments))))

(ert-gwt-deftest
  (:given ((text (make-string 200 (quote x))) (bounded nil)))
  (:when (setq bounded (pearl-tool-truncate text :limit 20)))
  (:then (<= (length (car bounded)) 40))
  (:then (cdr bounded)))

(ert-gwt-deftest
  (:given ((tool (pearl-tool-define "read" (quote (:description "read")) (lambda (_arguments _context) "ok"))) (name nil)))
  (:when (setq name (plist-get (car (pearl-tool-declarations (list tool))) :name)))
  (:then (equal "read" name)))
(provide 'tool-tests)

;;; tool-tests.el ends here
