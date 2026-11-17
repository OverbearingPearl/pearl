;;; tool-tests.el --- stories about the trust boundary  -*- lexical-binding: t; -*-

;;; Commentary:

;; A tool is where model text becomes an action, so this is where
;; arguments are checked once and where every failure stays a result.

;;; Code:

(require 'pearl-support-tests)
(require 'pearl-tool)
(require 'pearl-tools-read)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((tool (pearl-tool-define "boom" '(:description "always fails") (lambda (_arguments _context) (error "No such file")))) (result nil)))
  (:when (setq result (pearl-tool-run tool nil nil)))
  (:then (plist-get result :error)))

(ert-gwt-deftest
  (:given ((tool (pearl-tool-define "read" '(:description "read" :arguments ((path :required t))) (lambda (_arguments _context) "read"))) (arguments '((offset . 3)))))
  (:when (pearl-tool-validate tool arguments))
  (:then (should-error (pearl-tool-validate tool arguments))))

(ert-gwt-deftest
  (:given ((text (make-string 200 ?x)) (bounded nil)))
  (:when (setq bounded (pearl-tool-truncate text :limit 20)))
  (:then (<= (length (car bounded)) 40))
  (:then (cdr bounded)))

(ert-gwt-deftest
  (:given ((tool (pearl-tool-define "read" '(:description "read") (lambda (_arguments _context) "ok"))) (name nil)))
  (:when (setq name (plist-get (car (pearl-tool-declarations (list tool))) :name)))
  (:then (equal "read" name)))

;; Validation happens once, at the boundary: invalid arguments through
;; `pearl-tool-run' are an error result, never a signal the loop must catch.
(ert-gwt-deftest
  (:given ((tool (pearl-tool-define "read" '(:description "read" :arguments ((path :required t))) (lambda (_arguments _context) "read"))) (result nil)))
  (:when (setq result (pearl-tool-run tool '((offset . 3)) nil)))
  (:then (plist-get result :error)))

;; Valid arguments pass through unchanged, and the tool runs with them.
(ert-gwt-deftest
  (:given ((tool (pearl-tool-define "read" '(:description "read" :arguments ((path :required t))) (lambda (arguments _context) (alist-get 'path arguments)))) (result nil)))
  (:when (setq result (pearl-tool-run tool '((path . "a.el")) nil)))
  (:then (null (plist-get result :error)))
  (:then (equal "a.el" (plist-get result :content))))

;; A tool that wrote outside the session hands its patch up through the
;; result; the tool layer carries it without interpreting it.
(ert-gwt-deftest
  (:given ((patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (tool (pearl-tool-define "edit" '(:description "edit") (lambda (_arguments _context) (list :content "wrote" :patch patch))))
           (result nil)))
  (:when (setq result (pearl-tool-run tool nil nil)))
  (:then (equal patch (plist-get result :patch))))

;; A user escape inside a tool is an aborted result, not a signal that
;; escapes into the editor's command loop.
(ert-gwt-deftest
  (:given ((tool (pearl-tool-define "shell" '(:description "shell") (lambda (_arguments _context) (signal 'quit nil)))) (result nil)))
  (:when (setq result (pearl-tool-run tool nil nil)))
  (:then (plist-get result :error)))

;; A tail window keeps the end of the output and says it is a fragment:
;; the last lines are what a build failure is read from.
(ert-gwt-deftest
  (:given ((text "0123456789abcde") (bounded nil)))
  (:when (setq bounded (pearl-tool-truncate text :tail 5)))
  (:then (cdr bounded))
  (:then (string-suffix-p "abcde" (car bounded))))

;; Output under the limit is untouched and unmarked.
(ert-gwt-deftest
  (:given ((text "short") (bounded nil)))
  (:when (setq bounded (pearl-tool-truncate text :limit 20)))
  (:then (equal "short" (car bounded)))
  (:then (null (cdr bounded))))

;; The registry is a variable an extension rebinds, and installation is
;; composition: no discovery step, no registry object (ADR 6).
(ert-gwt-deftest
  (:given ((pearl-tool-registry (list (pearl-tool-define "custom" '(:description "mine") #'ignore))) (installed nil)))
  (:when (setq installed (pearl-tools-read-install (pearl-tool-list))))
  (:then (equal '("custom") (mapcar (lambda (tool) (plist-get tool :name)) (pearl-tool-list))))
  (:then (equal '("custom" "read") (mapcar (lambda (tool) (plist-get tool :name)) installed))))

(provide 'tool-tests)

;;; tool-tests.el ends here
