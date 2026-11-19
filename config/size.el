;;; size.el --- The definition count `make size' prints  -*- lexical-binding: t; -*-

;;; Commentary:

;; `ci/size.el' counts lines and repeated runs for every project; a definition
;; is what this package can count and no other project means the same thing by
;; it, so the column arrives as a function the suite asks once per module.
;;
;; It has to come from `scripts/contracts.el', which already reads every
;; definition to audit the promises: a second regexp for the same shape would
;; drift, and the number nobody can check twice is worse than no number.  So
;; this file is a hook and a cache, not a table of thresholds -- the policy it
;; complements stays in the `:size' entry of `config/ci.el'.

;;; Code:

(defvar p/size-definitions)

(let* ((file (or load-file-name buffer-file-name))
       (here (file-name-directory file)))
  (add-to-list 'load-path (expand-file-name "../scripts" here)))

(defvar pearl-size-counts nil
  "Hash of absolute file name to definition count, or nil until it is built.
`contracts-definitions' walks the whole tree, so it is asked once and the
answer kept: the report asks per module, and a parse per row would be the
slowest thing in it.")

(defun pearl-size-definitions-in (file)
  "Return how many definitions FILE carries.
Both sides are truenamed because `contracts-definitions' spells its files
from the repository root while the report is handed the paths its module
table produced; a symlinked checkout is enough to make those disagree."
  (unless pearl-size-counts
    (require 'contracts)
    (setq pearl-size-counts (make-hash-table :test #'equal))
    (dolist (entry (contracts-definitions))
      (let ((key (file-truename (cdr entry))))
        (puthash key (1+ (gethash key pearl-size-counts 0)) pearl-size-counts))))
  (gethash (file-truename file) pearl-size-counts 0))

(setq p/size-definitions #'pearl-size-definitions-in)

(provide 'pearl-size)

;;; size.el ends here
