;;; 09-module-prefix.el --- definition naming rules -*- lexical-binding: t; -*-

;;; Commentary:

;; Definitions must be namespaced.  A function, variable or macro is
;; accepted when it starts with the module prefix (`<file-base>-'), or
;; with the package private prefix (`<package>--'), which is the
;; convention for helpers shared across modules.  defcustom is public
;; API, so it must use `<package>-' and must not use the private prefix.
;; The root main file may define an entry command named after the
;; package.  Minor modes use `<package>-...-mode'.  Test modules are
;; exempt: their definitions need no repository-derived prefix.

;;; Code:

(defun p/minor-mode-name-ok-p (name repo-prefix)
  "Return non-nil when NAME is a valid minor mode name."
  (and (string-prefix-p repo-prefix name)
       (string-suffix-p "-mode" name)))

(defun p/check-definition-name (check file line kind name repo is-main)
  "Report NAME at LINE when it is not namespaced.
KIND is the defining form, REPO the package name, and CHECK the check
requesting the report."
  (let* ((nm (symbol-name name))
         (repo-prefix (concat repo "-"))
         (private-prefix (concat repo "--"))
         (file-prefix (concat (file-name-base file) "-")))
    (cond
     ((and is-main (eq kind 'defun) (string= nm repo))
      nil)
     ((eq kind 'defcustom)
      (cond
       ((string-prefix-p private-prefix nm)
        (p/add-error check file line
                     (format "defcustom `%s' must not use the private prefix `%s'"
                             name private-prefix)))
       ((not (string-prefix-p repo-prefix nm))
        (p/add-error check file line
                     (format "defcustom `%s' must start with `%s'"
                             name repo-prefix)))))
     ((not (or (string-prefix-p file-prefix nm)
               (string-prefix-p private-prefix nm)))
      (p/add-error check file line
                   (format "`%s' must start with `%s' or `%s'"
                           name file-prefix private-prefix))))))

(p/defcheck module-prefix :classes (main business)
  "Definitions must be namespaced; test and tooling modules are exempt."
  (let* ((repo (p/package-name))
         (repo-prefix (concat repo "-"))
         (private-prefix (concat repo "--"))
         (file-prefix (concat (file-name-base file) "-"))
         (def-kinds (p/config-value :def-kinds))
         (is-main (eq class 'main)))
    (pcase-dolist (`(,form . ,line) (p/top-level-forms file))
      (let ((kind (and (consp form) (car form)))
            (name (and (consp form) (cadr form))))
        (when (and kind (memq kind def-kinds) (symbolp name))
          (p/check-definition-name check file line kind name repo is-main))
        (when (and (eq kind 'define-minor-mode) (symbolp name))
          (unless (p/minor-mode-name-ok-p (symbol-name name) repo-prefix)
            (p/add-error check file line
                         (format "`%s' minor mode must start with `%s' and end with `-mode'"
                                 name repo-prefix))))))))
