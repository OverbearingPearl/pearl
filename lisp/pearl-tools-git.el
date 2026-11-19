;;; pearl-tools-git.el --- Committing the paths a turn names   -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; A commit is the one write whose scope is not the file it touches.  The
;; shell tool refuses git on an unattended run because a `git add -A' that
;; ran while another session was halfway through an edit commits that edit
;; under this turn's message, and nothing in the result says so.  The tool
;; this module offers is the narrow surface that makes the opposite
;; possible: it stages the paths the model names, and nothing else.
;;
;; `pearl-tools-git-commit' (ARGUMENTS CONTEXT) -> result
;;      Commit the paths ARGUMENTS names, in the root CONTEXT names.  A path
;;      list is required: a commit that stages what it was not told to stage
;;      is the failure the surface exists to prevent.  The tool refuses an
;;      index that already holds something, because adding on top of another
;;      session's `git add' would carry that session's file as if this turn
;;      had written it.  A `:gate' names the epoch a passing verdict was
;;      about, as the note a shell result carries: a path that moved after
;;      it is refused rather than committed under a verdict the gate never
;;      gave.  The result carries `:commit' (the new head), `:epoch' (the
;;      fingerprint it was committed over) and `:gate', and a refused commit
;;      leaves the index as it found it.
;;
;; `pearl-tools-git-install' (REGISTRY) -> registry
;;      Add the tool to the list an extension may replace (ADR 6).

;;; Code:

(require 'cl-lib)
(require 'pearl-contract)
(require 'pearl-proc)
(require 'pearl-tool)
(require 'pearl-tree)

(defcustom pearl-tools-git-timeout 60
  "How long a git command may take, in seconds."
  :group 'pearl
  :type 'integer)

(defun pearl-tools-git--call ()
  "The call a git result carries."
  (list :name "commit"))

(defun pearl-tools-git--failure (message)
  "A git result carrying MESSAGE as the reason it did nothing."
  (pearl-contract-tool-result (pearl-tools-git--call) message t))

(defun pearl-tools-git--paths (paths)
  "Return PATHS as a list of strings, or nil.
A model shown one argument may send a string with the paths in it; a
newline or a space is the boundary it used."
  (cond ((not paths) nil)
        ((stringp paths) (split-string paths "[ \t\n]+" t))
        ((consp paths)
         (delq nil (mapcar (lambda (path) (and (stringp path) path)) paths)))
        (t nil)))

(defun pearl-tools-git--git-raw (root &rest arguments)
  "Run git with ARGUMENTS in ROOT, and return `(STATUS . STDOUT)'.
A file's content is not trimmed: a trailing newline is part of what the
commit carries."
  (let* ((process-environment (pearl-tree-environment))
         (result (pearl-proc-run "git" (append (list "-C" root) arguments)
                                 :timeout pearl-tools-git-timeout))
         (status (if (plist-get result :timeout) 1 (plist-get result :status))))
    (cons (if (integerp status) status 1)
          (or (plist-get result :stdout) ""))))

(defun pearl-tools-git--git (root &rest arguments)
  "Run git with ARGUMENTS in ROOT.
Return `(STATUS . OUTPUT)' where OUTPUT is stdout and stderr together,
because git says which path it refused on stderr and a refusal is what
the caller has to repeat."
  (let ((run (apply #'pearl-tools-git--git-raw root arguments)))
    (cons (car run) (string-trim (cdr run)))))

(defun pearl-tools-git--staged (root)
  "Return the paths in ROOT's index, in git's order."
  (let ((lines (cdr (pearl-tools-git--git root "diff" "--cached" "--name-only"))))
    (split-string lines "\n" t)))

(defun pearl-tools-git--content (root path)
  "Return PATH's text in ROOT's working tree, or nil when it is gone."
  (let ((file (expand-file-name path root)))
    (when (file-regular-p file)
      (with-temp-buffer
        (insert-file-contents-literally file)
        (buffer-string)))))

(defun pearl-tools-git--contents (root paths hunks)
  "Return what a commit of PATHS or HUNKS in ROOT would carry, as an alist.
The value is `(PATH . TEXT)' per file, or a plist with `:problem' when a
hunk cannot be applied.  This is the text, not the file's stamp: a gate is
a verdict about a text, and the commit is the same text or it is not."
  (if hunks
      (let ((contents '()) (problem nil))
        (dolist (path (delete-dups (mapcar (lambda (hunk) (plist-get hunk :file))
                                           hunks)))
          (unless problem
            (let ((text (pearl-tools-git--hunk-text
                         root path (seq-filter (lambda (hunk)
                                                 (equal path (plist-get hunk :file)))
                                               hunks))))
              (if (listp text)
                  (setq problem (plist-get text :problem))
                (push (cons path text) contents)))))
        (if problem (list :problem problem) (nreverse contents)))
    (mapcar (lambda (path) (cons path (pearl-tools-git--content root path))) paths)))

(defun pearl-tools-git--moved-since (gate contents)
  "Return the paths in CONTENTS the tree GATE names does not cover.
A path the gate saw with another text is a path the verdict is not about;
a path the gate did not see at all was at HEAD's text when it ran, so a
commit that changes it is a commit the verdict never covered."
  (let ((before (plist-get gate :changed)))
    (seq-filter
     (lambda (path)
       (let* ((entry (assoc path before))
              (text (cdr (assoc path contents)))
              (hash (and text (secure-hash 'sha256 text))))
         (not (and entry (equal hash (nth 3 entry))))))
     (mapcar #'car contents))))

(defun pearl-tools-git--unstage (root paths)
  "Take PATHS back out of ROOT's index, leaving the worktree alone.
A refused commit must leave the index as it found it, which is empty:
`git reset' on a repository with no history fails, and that failure is
not one the caller can do anything about."
  (apply #'pearl-tools-git--git root (append (list "reset" "-q" "--") paths))
  nil)

(defun pearl-tools-git--same-paths-p (one other)
  "Non-nil when ONE and OTHER are the same paths, in any order."
  (equal (sort (copy-sequence one) #'string<)
         (sort (copy-sequence other) #'string<)))

(defun pearl-tools-git--base (root path)
  "Return PATH as HEAD carries it in ROOT, or nil when HEAD has none."
  (let ((run (pearl-tools-git--git-raw root "show" (concat "HEAD:" path))))
    (when (zerop (car run))
      (cdr run))))

(defun pearl-tools-git--hash-object (root text)
  "Write TEXT into ROOT's object store and return its blob hash."
  (let* ((process-environment (pearl-tree-environment))
         (run (pearl-proc-run "git" (list "-C" root "hash-object" "-w" "--stdin")
                              :input text
                              :timeout pearl-tools-git-timeout)))
    (string-trim (or (plist-get run :stdout) ""))))

(defun pearl-tools-git--occurrences (text anchor)
  "Count how often ANCHOR appears in TEXT."
  (let ((count 0) (position 0))
    (while (and (< position (length text))
                (string-match (regexp-quote anchor) text position))
      (setq count (1+ count) position (match-end 0)))
    count))

(defun pearl-tools-git--apply (text anchor replacement)
  "Return TEXT with ANCHOR replaced by REPLACEMENT.
Nil means the anchor is not there, `ambiguous' that it is there more than
once: an anchor with two homes is not a hunk, and picking one of them is
how a commit carries a region nobody named."
  (pcase (pearl-tools-git--occurrences text anchor)
    (0 nil)
    (1 (replace-regexp-in-string (regexp-quote anchor) replacement text t t))
    (_ 'ambiguous)))

(defun pearl-tools-git--hunks (value)
  "Return VALUE as a list of hunks, or nil when it is not one.
A hunk is a plist naming a file and the text it replaces: the anchor rule
ADR 4 gives an edit, applied to a commit instead of to a buffer."
  (let ((hunks (cond ((null value) nil)
                     ((stringp value) nil)
                     ((and (consp value) (not (consp (car value)))) (list value))
                     (t value)))
        (good t))
    (dolist (hunk hunks)
      (unless (and (listp hunk)
                   (stringp (plist-get hunk :file))
                   (stringp (plist-get hunk :old-text))
                   (stringp (plist-get hunk :new-text)))
        (setq good nil)))
    (and good (consp hunks) hunks)))

(defun pearl-tools-git--hunk-text (root path hunks)
  "Return PATH's content in ROOT with HUNKS applied, or a plist saying why not.
The base is HEAD, not the working tree: what a commit carries is what the
last commit had plus the regions named, and the rest of the file - another
session's region, a half-written edit - stays where it is."
  (let ((base (pearl-tools-git--base root path)))
    (if (not base)
        (list :problem (format "%s is not in HEAD; a new file is committed whole" path))
      (let ((text base) (problem nil))
        (dolist (hunk hunks)
          (unless problem
            (let ((next (pearl-tools-git--apply
                         text (plist-get hunk :old-text) (plist-get hunk :new-text))))
              (cond ((eq next 'ambiguous)
                     (setq problem
                           (format "the text a hunk names appears more than once in HEAD's %s" path)))
                    ((null next)
                     (setq problem
                           (format "the text a hunk names is not in HEAD's %s" path)))
                    (t (setq text next))))))
        (if problem (list :problem problem) text)))))

(defun pearl-tools-git--stage-hunks (root contents)
  "Stage CONTENTS in ROOT's index, one blob per file.
CONTENTS is the alist `pearl-tools-git--contents' returns.  Return
`(PATHS . PROBLEM)': the paths staged, or the reason nothing was."
  (let ((paths (mapcar #'car contents))
        (problem nil))
    (dolist (entry contents)
      (unless problem
        (let* ((path (car entry))
               (blob (pearl-tools-git--hash-object root (cdr entry)))
               (staged (pearl-tools-git--git
                        root "update-index" "--add" "--cacheinfo"
                        "100644" blob path)))
          (unless (zerop (car staged))
            (setq problem (format "git could not stage %s: %s"
                                  path (cdr staged)))))))
    (cons paths problem)))

(defun pearl-tools-git--commit (root message paths epoch gate hunks contents)
  "Commit PATHS in ROOT with MESSAGE, over the tree EPOCH names.
When HUNKS is non-nil CONTENTS is staged as blobs; otherwise PATHS are
staged whole and CONTENTS is what that stages."
  (let* ((staged (if hunks
                     (pearl-tools-git--stage-hunks root contents)
                   (let ((added (apply #'pearl-tools-git--git root
                                       (append (list "add" "--") paths))))
                     (if (zerop (car added))
                         (cons paths nil)
                       (cons nil (format "git could not stage %s: %s"
                                         (string-join paths ", ") (cdr added)))))))
         (staged-paths (car staged))
         (problem (cdr staged)))
    (cond
     (problem
      (pearl-tools-git--unstage root staged-paths)
      (pearl-tools-git--failure (format "Nothing was committed: %s" problem)))
     ((not (pearl-tools-git--same-paths-p
            (pearl-tools-git--staged root) staged-paths))
      (pearl-tools-git--unstage root staged-paths)
      (pearl-tools-git--failure
       (format "Nothing was committed: staging %s left the index holding %s, which is not what this commit named"
               (string-join staged-paths ", ")
               (string-join (pearl-tools-git--staged root) ", "))))
     (t
      (let ((run (apply #'pearl-tools-git--git root
                        (list "commit" "-m" message))))
        (if (not (zerop (car run)))
            (progn
              (pearl-tools-git--unstage root staged-paths)
              (pearl-tools-git--failure
               (format "Nothing was committed: %s" (cdr run))))
          (let ((head (cdr (pearl-tools-git--git root "rev-parse" "HEAD"))))
            (append (pearl-contract-tool-result
                     (pearl-tools-git--call)
                     (format "Committed %s: %s"
                             (substring head 0 7)
                             (car (split-string message "\n"))))
                    (list :commit head :epoch epoch :paths staged-paths
                          :gate gate)))))))))

(defun pearl-tools-git-commit (arguments context)
  "Commit the paths ARGUMENTS names, in the root CONTEXT names.
A `:hunks' argument names regions instead: each is a file and the text it
replaces, and what is committed is HEAD's copy of that file with those
regions applied - the rest of the working tree, including another
session's region in the same file, stays out of the commit."
  (let* ((arguments (pearl-tool-normalize-arguments arguments))
         (message (plist-get arguments :message))
         (hunks-value (plist-get arguments :hunks))
         (hunks (and hunks-value (pearl-tools-git--hunks hunks-value)))
         (paths (pearl-tools-git--paths
                 (or (and hunks
                          (delete-dups
                           (mapcar (lambda (hunk) (plist-get hunk :file)) hunks)))
                     (plist-get arguments :paths))))
         (note (plist-get arguments :gate))
         (gate (and note (pearl-tree-epoch-from-note note)))
         (root (plist-get context :root))
         (epoch (and root (pearl-tree-epoch root)))
         (contents (and root paths (pearl-tools-git--contents root paths hunks)))
         (moved (and gate (listp contents) (pearl-tools-git--moved-since gate contents)))
         (staged (and root (pearl-tools-git--staged root))))
    (cond
     ((null root)
      (pearl-tools-git--failure "Nothing was committed: there is no project root"))
     ((not (and (stringp message) (not (string-empty-p message))))
      (pearl-tools-git--failure "Nothing was committed: a commit needs a message"))
     ((and hunks-value (not hunks))
      (pearl-tools-git--failure
       "Nothing was committed: a hunk needs file, oldText and newText; a region with no anchor is not a region"))
     ((and hunks-value (plist-get arguments :paths))
      (pearl-tools-git--failure
       "Nothing was committed: name paths or hunks, not both; a hunk already names its file"))
     ((null paths)
      (pearl-tools-git--failure
       "Nothing was committed: paths or hunks is required; a commit stages what it names and nothing else"))
     (staged
      (pearl-tools-git--failure
       (format "Nothing was committed: the index already holds %s, which this commit did not name"
               (string-join staged ", "))))
     ((and note (not gate))
      (pearl-tools-git--failure
       "Nothing was committed: the gate names no tree; hand back the note the result carried"))
     ((and contents (plist-get contents :problem))
      (pearl-tools-git--failure
       (format "Nothing was committed: %s" (plist-get contents :problem))))
     (moved
      (pearl-tools-git--failure
       (format "Nothing was committed: %s is not the text the gate ran on; run the gate again"
               (string-join moved ", "))))
     (t (pearl-tools-git--commit root message paths epoch note hunks contents)))))

(defun pearl-tools-git-declaration ()
  "Return the declaration the model is shown for a commit."
  (pearl-tool-define
   "commit"
   '(:description "Commit the named paths or hunks and nothing else: no staged file, no other session's region."
     :arguments ((message :required t :description "The commit message, subject first")
                 (paths :description "Paths to stage and commit whole, one per entry")
                 (hunks :description "Regions to stage instead of whole paths: each names a file, the text it replaces and the text it becomes; another region of the same file stays out")
                 (gate :description "The gate note a passing result carried, copied verbatim")))
   #'pearl-tools-git-commit))

(defun pearl-tools-git-install (registry)
  "REGISTRY with the commit tool added."
  (append registry (list (pearl-tools-git-declaration))))

(provide 'pearl-tools-git)

;;; pearl-tools-git.el ends here
