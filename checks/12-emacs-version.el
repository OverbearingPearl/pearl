;;; 12-emacs-version.el --- running Emacs floor -*- lexical-binding: t; -*-

;;; Commentary:

;; The Emacs running the suite must satisfy the configured minimum, so a
;; CI runner or a contributor cannot lint with an older Emacs than the
;; package declares.

;;; Code:

(p/defcheck-global emacs-version-minimum :scope "the running Emacs"
  "The running Emacs must satisfy the configured minimum version."
  (let ((minimum (car (p/config-value :emacs-versions))))
    (when (and minimum
               (condition-case nil
                   (version< emacs-version minimum)
                 (error nil)))
      (p/add-error check (p/main-file) nil
                   (format "running Emacs %s is below the configured minimum %s"
                           emacs-version minimum)))))
