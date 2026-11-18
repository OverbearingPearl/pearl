;;; contracts.el --- check the contracts against the stories -*- lexical-binding: t; -*-

;;; Commentary:

;; Every function this package promises is written down twice: once in its
;; module as a `;; TODO' line giving the name, the arguments and the value,
;; and once in `test/support-tests.el' as a stub, so that a story fails on
;; the story rather than on a void function. The third place is the story
;; itself, which is the specification.
;;
;; None of the three is checked against the others, and all three drift the
;; same quiet way: a name in the map that no module promises, a promise no
;; story mentions, and an argument list in a comment that disagrees with the
;; call in the test. The comment is the copy nobody compiles, which is why it
;; is the one that is wrong.
;;
;; This reads the three and reports the disagreements in both directions. It
;; is not a check in `ci/' -- that suite is a shared subtree and it knows
;; nothing about this project's way of writing a promise down.

;;; Code:

(require 'cl-lib)
(require 'subr-x)

(defconst contracts-todo-rx
  (rx line-start ";; TODO `" (group (+ (not ?'))) "'" (group (* nonl)))
  "Regexp matching a promise, capturing its name and what follows.

What follows says which kind of promise it is, because the two are stubbed
and proven differently: a function is written `(ARGS) -> RET' and needs a
stub so that a story can call it; a variable is written `-> defcustom' or
`-> defconst' and is data a story reads without one.")

(defconst contracts-stub-rx
  (rx "(support-tests--stub " (group (+ (not (any " \t\n()")))) ")")
  "Regexp matching a stub entry in the map.")

(defconst contracts-root
  (file-name-directory
   (directory-file-name
    (file-name-directory (or load-file-name buffer-file-name default-directory))))
  "The repository root, resolved once when this file is loaded.
`load-file-name' is bound only while loading, so a function that read it
would return nil the moment a caller ran.")

(defun contracts-files (dir)
  "Return the Lisp files directly inside DIR, sorted."
  (sort (seq-filter (lambda (f) (string-suffix-p ".el" f))
                    (directory-files dir t))
        #'string<))

(defun contracts-function-p (tail)
  "Return non-nil when a promise's TAIL describes a function.
A function is written with an argument list before the arrow; a variable
is written with its storage kind after it."
  (and (string-match-p (rx "(" (* nonl) ")" (* blank) "->") tail)
       (not (string-match-p (rx "->" (* blank) (or "defcustom" "defconst"))
                            tail))))

(defun contracts-promises ()
  "Return (NAME . FILE) for every promise in the business modules.
Each entry also carries whether the name is internal, which is what decides
whether a story has to name it."
  (let ((promises '()))
    (dolist (file (contracts-files (expand-file-name "lisp" contracts-root)))
      (with-temp-buffer
        (insert-file-contents file)
        (goto-char (point-min))
        (while (re-search-forward contracts-todo-rx nil t)
          (push (list (intern (match-string 1)) file
                      (contracts-function-p (match-string 2)))
                promises))))
    (nreverse promises)))

(defun contracts-internal-p (name)
  "Return non-nil when NAME is a private helper.
Two hyphens after the prefix is the Emacs convention, and a helper is
reached through its caller rather than named by a story."
  (string-match-p (rx "--") (symbol-name name)))

(defun contracts-stubs ()
  "Return the names the stub map lists."
  (let ((path (expand-file-name "test/support-tests.el" contracts-root))
        (names '()))
    (when (file-regular-p path)
      (with-temp-buffer
        (insert-file-contents path)
        (goto-char (point-min))
        (while (re-search-forward contracts-stub-rx nil t)
          (push (intern (match-string 1)) names))))
    (nreverse names)))

(defun contracts-story-files ()
  "Return the test modules, which are where the stories live."
  (seq-filter (lambda (f) (not (string-suffix-p "support-tests.el" f)))
              (contracts-files (expand-file-name "test" contracts-root))))

(defun contracts-mentioned ()
  "Return a hash of every symbol named in a story.
The whole file is scanned rather than the calls, because a story names a
promise in its comment as often as in its `:when', and either is proof
that somebody wrote the story down."
  (let ((table (make-hash-table :test #'eq)))
    (dolist (file (contracts-story-files))
      (with-temp-buffer
        (insert-file-contents file)
        (goto-char (point-min))
        (while (re-search-forward (rx (group (or "pearl" "support-tests")
                                            (+ (or alnum ?- ?/))))
                                  nil t)
          (puthash (intern (match-string 1)) t table))))
    table))


(defconst contracts-definition-rx
  (rx line-start "(" (group (or "defun" "defmacro" "defsubst" "defalias"
                                "cl-defun" "cl-defmacro" "define-minor-mode"))
      (+ blank) (group (+ (or alnum ?- ?+ ?* ?/ ?! ?< ?> ?=))))
  "Regexp matching a top-level definition, capturing its name.")

(defun contracts-business-files ()
  "Return the business modules: `lisp/' and the entry file."
  (append (contracts-files (expand-file-name "lisp" contracts-root))
          (seq-filter #'file-regular-p
                      (list (expand-file-name "pearl.el" contracts-root)))))

(defun contracts-count (name files)
  "Return how often a call to NAME appears across FILES.
Only call shapes count -- `(NAME', `#\'NAME' and
`\='NAME' -- because a name inside backquotes is a docstring telling a
reader about it, not a caller."
  (let ((rx (rx (or "(" "#\'" "\='")
                (literal (symbol-name name))
                (or blank ?\) line-end)))
        (count 0))
    (dolist (file files)
      (with-temp-buffer
        (insert-file-contents file)
        (goto-char (point-min))
        (while (re-search-forward rx nil t) (setq count (1+ count)))))
    count))

(defun contracts-definitions ()
  "Return (NAME . FILE) for every function the business modules define.
A minor mode is left out: a user types its name, so it has no caller by
design, and so is the package's own entry command for the same reason."
  (let ((found '()))
    (dolist (file (contracts-business-files))
      (with-temp-buffer
        (insert-file-contents file)
        (goto-char (point-min))
        (while (re-search-forward contracts-definition-rx nil t)
          (let ((name (intern (match-string 2))))
            (unless (or (string-match-p (rx "-mode\'") (symbol-name name))
                        (eq name 'pearl))
              (push (cons name file) found))))))
    (nreverse found)))

(defun contracts-uncalled ()
  "Return (NAME FILE BUSINESS TESTS) for definitions nothing calls.
BUSINESS is the number of calls in the modules themselves, TESTS the
number in the stories.  A zero in the first is the shape this report
exists for: a mechanism that was written, proved, and never wired to
anything a user can reach."
  (let ((tests (contracts-story-files))
        (business (contracts-business-files))
        (uncalled '()))
    (dolist (definition (contracts-definitions))
      (let* ((name (car definition))
             (in-business (contracts-count name business))
             (in-tests (contracts-count name tests)))
        (when (zerop in-business)
          (push (list name (cdr definition) in-business in-tests) uncalled))))
    (nreverse uncalled)))

(defun contracts-report ()
  "Print the disagreements; return non-nil when there are none."
  (let* ((promises (contracts-promises))
         (promised (mapcar #'car promises))
         (stubs (contracts-stubs))
         (stories (contracts-mentioned))
         (problems 0))
    (dolist (name stubs)
      (unless (assq name promises)
        (setq problems (1+ problems))
        (princ (format "test/support-tests.el: `%s' has a stub and no promise\n"
                       name))))
    (dolist (promise promises)
      (let ((name (car promise))
            (file (file-relative-name (cadr promise) contracts-root))
            (functionp (caddr promise)))
        (when (and functionp
                   (not (contracts-internal-p name))
                   (not (memq name stubs)))
          (setq problems (1+ problems))
          (princ (format "%s: `%s' is promised and has no stub\n" file name)))
        (when (and functionp
                   (not (contracts-internal-p name))
                   (not (gethash name stories)))
          (setq problems (1+ problems))
          (princ (format "%s: `%s' is promised and no story names it\n"
                         file name)))))
    (let ((orphans 0) (unwired 0))
      (dolist (entry (contracts-uncalled))
        (if (> (nth 3 entry) 0)
            (setq unwired (1+ unwired))
          (setq orphans (1+ orphans))
          (setq problems (1+ problems))
          (princ (format "%s: `%s' is defined and nothing calls it\n"
                         (file-relative-name (nth 1 entry) contracts-root)
                         (nth 0 entry)))))
      (princ (format "contracts: %d defined, %d unwired, %d with no caller at all\n"
                     (length (contracts-definitions)) unwired orphans)))
    (princ (format "contracts: %d promises, %d stubs, %d stories, %d disagreements\n"
                   (length promised) (length stubs) (length (contracts-story-files))
                   problems))
    (zerop problems)))

(provide 'contracts)

;;; contracts.el ends here
