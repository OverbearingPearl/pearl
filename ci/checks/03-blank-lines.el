;;; 03-blank-lines.el --- blank line rule -*- lexical-binding: t; -*-

;;; Commentary:

;; At most `:blank-lines' consecutive blank lines, nil meaning the project
;; has no opinion.  Whitespace hygiene for every file is handled by the
;; global `text-hygiene' check.
;;
;; The number is policy and not mechanism: it is the one styling rule in the
;; suite with no upstream to point at, so a project that likes two blank
;; lines between top-level forms -- which is ordinary Emacs Lisp -- says two
;; in its own config instead of deleting the check.

;;; Code:

(p/defcheck blank-lines
  "At most `:blank-lines' consecutive blank lines."
  (let ((max (p/config-value :blank-lines)))
    (when max
      (let ((line-num 0)
            (blank-run 0)
            (last-blank-start nil))
        (dolist (line (p/file-lines file))
          (setq line-num (1+ line-num))
          (if (p/blank-line-p line)
              (progn
                (when (= blank-run 0)
                  (setq last-blank-start line-num))
                (setq blank-run (1+ blank-run)))
            (progn
              (when (> blank-run max)
                (p/add-error check file last-blank-start
                             (format "%d consecutive blank lines, more than %d"
                                     blank-run max)))
              (setq blank-run 0))))
        (when (> blank-run max)
          (p/add-error check file last-blank-start
                       (format "%d consecutive blank lines, more than %d"
                               blank-run max)))))))
