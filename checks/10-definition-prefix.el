;;; 10-definition-prefix.el --- definitions carry the module name -*- lexical-binding: t; -*-

;;; Commentary:

;; Every top-level definition is namespaced by the module it lives in: the
;; prefix is the module's full name, the file name without `.el', not just
;; the package name.  `lisp/<pkg>-loop.el' therefore defines
;; `pkg-loop-...', and `test/loop-tests.el' defines `loop-tests-...'.
;;
;; The rule is the same for functions, variables, macros, aliases,
;; customisations and ERT tests, and minor modes additionally end in
;; `-mode'.  Two exemptions:
;;
;;   - the main module may define an entry command named exactly the
;;     package, because that is what users type;
;;   - tooling modules are shared code that must not name the package at
;;     all (see `project-neutral'), so their definitions are free.
;;
;; Vendored code keeps its own names and is not checked.

;;; Code:

(defun p/definition-prefix (file)
  "Return the prefix a definition in FILE must carry."
  (concat (file-name-base file) "-"))

(defun p/main-entry-command-p (name kind is-main repo)
  "Return non-nil when NAME is the main module's entry command.
KIND is the defining form, IS-MAIN says the module is the main one, and
REPO is the package name."
  (and is-main (eq kind 'defun) (string= name repo)))

(defun p/minor-mode-name-ok-p (name prefix)
  "Return non-nil when NAME is a minor mode under PREFIX."
  (and (string-prefix-p prefix name)
       (string-suffix-p "-mode" name)))

(p/defcheck definition-prefix :classes (main business test)
  "Every definition starts with the name of the module it lives in."
  (let* ((prefix (p/definition-prefix file))
         (repo (p/package-name))
         (is-main (eq class 'main))
         (def-kinds (p/config-value :def-kinds)))
    (pcase-dolist (`(,form . ,line) (p/top-level-forms file))
      (when (consp form)
        (let ((kind (car form))
              (name (cadr form)))
          (when (symbolp name)
            (let ((nm (symbol-name name)))
              (cond
               ((memq kind def-kinds)
                (unless (or (p/main-entry-command-p nm kind is-main repo)
                            (string-prefix-p prefix nm))
                  (p/add-error check file line
                               (format "`%s' must start with `%s'"
                                       name prefix))))
               ((eq kind 'define-minor-mode)
                (unless (p/minor-mode-name-ok-p nm prefix)
                  (p/add-error check file line
                               (concat "minor mode `" name
                                       "' must start with `" prefix
                                       "' and end with `-mode'"))))))))))))
