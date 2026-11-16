;;; 00-checkdoc.el --- checkdoc on package modules -*- lexical-binding: t; -*-

;;; Commentary:

;; Runs checkdoc-current-buffer with autofix disabled and collects every
;; reported issue instead of stopping at the first one.

;;; Code:

(require 'checkdoc)
(require 'cl-lib)

(p/defcheck checkdoc :classes (main business test)
  "checkdoc-current-buffer with autofix disabled."
  (let ((checkdoc-autofix-flag (p/config-value :checkdoc-autofix))
        (orig (symbol-function 'checkdoc-create-error))
        (collected '()))
    (cl-letf (((symbol-function 'checkdoc-create-error)
               (lambda (&rest args)
                 (let* ((msg (car args))
                        (pos (or (and (numberp (nth 1 args)) (nth 1 args))
                                 (and (numberp (nth 2 args)) (nth 2 args))))
                        (line (and pos (line-number-at-pos pos))))
                   (push (if line (format "%s (line %d)" msg line) msg)
                         collected))
                 (apply orig args))))
      (p/with-file-buffer
       file
       (lambda ()
         (condition-case nil
             (checkdoc-current-buffer t)
           (error nil)))))
    (dolist (e collected)
      (p/add-error check file nil e))))
