;;; 03-blank-lines.el --- blank line rule -*- lexical-binding: t; -*-

;;; Commentary:

;; At most one consecutive blank line.  Whitespace hygiene for every file
;; is handled by the global `text-hygiene' check.

;;; Code:

(p/defcheck blank-lines
  "At most one consecutive blank line."
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
          (when (> blank-run 1)
            (p/add-error check file last-blank-start
                         "more than one consecutive blank line"))
          (setq blank-run 0))))
    (when (> blank-run 1)
      (p/add-error check file last-blank-start
                   "more than one consecutive blank line"))))
