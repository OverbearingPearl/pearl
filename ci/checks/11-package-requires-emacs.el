;;; 11-package-requires-emacs.el --- emacs floor declaration -*- lexical-binding: t; -*-

;;; Commentary:

;; The main file's Package-Requires emacs dependency must be at least
;; the configured minimum.

;;; Code:

(p/defcheck-global package-requires-emacs :scope "the main file"
  "The declared emacs requirement must meet the configured minimum."
  (let* ((main (p/main-file))
         (min-v (car (p/config-value :emacs-versions)))
         (declared (p/package-requires-emacs-version main)))
    (cond
     ((null declared)
      (p/add-error check main nil
                   (format "missing Package-Requires emacs dependency (expected >= %s)"
                           min-v)))
     ((version< declared min-v)
      (p/add-error check main nil
                   (format "Package-Requires emacs %s is below minimum %s"
                           declared min-v))))))
