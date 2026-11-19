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

(defun pearl-tools-git--git (root &rest arguments)
  "Run git with ARGUMENTS in ROOT.
Return `(STATUS . OUTPUT)' where OUTPUT is stdout and stderr together,
because git says which path it refused on stderr and a refusal is what
the caller has to repeat."
  (let* ((result (pearl-proc-run "git" (append (list "-C" root) arguments)
                                 :timeout pearl-tools-git-timeout))
         (status (if (plist-get result :timeout) 1 (plist-get result :status)))
         (output (string-trim
                  (concat (plist-get result :stdout) (plist-get result :stderr)))))
    (cons (if (integerp status) status 1) output)))

(defun pearl-tools-git--staged (root)
  "Return the paths in ROOT's index, in git's order."
  (let ((lines (cdr (pearl-tools-git--git root "diff" "--cached" "--name-only"))))
    (split-string lines "\n" t)))

(defun pearl-tools-git--moved-since (gate paths epoch)
  "Return the paths among PATHS that moved between GATE and EPOCH.
A gate is a verdict about the tree that was there when it ran; a path whose
size or modification time is not the one the gate saw is a path the verdict
does not cover, whichever session moved it."
  (let ((before (plist-get gate :changed))
        (after (plist-get epoch :changed)))
    (seq-filter (lambda (path)
                  (not (equal (assoc path before) (assoc path after))))
                paths)))

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

(defun pearl-tools-git--commit (root message paths epoch gate)
  "Commit PATHS in ROOT with MESSAGE, over the tree EPOCH names."
  (let ((added (apply #'pearl-tools-git--git root
                      (append (list "add" "--") paths))))
    (cond
     ((not (zerop (car added)))
      (pearl-tools-git--failure
       (format "Nothing was committed: git could not stage %s: %s"
               (string-join paths ", ") (cdr added))))
     ((not (pearl-tools-git--same-paths-p (pearl-tools-git--staged root) paths))
      (pearl-tools-git--unstage root paths)
      (pearl-tools-git--failure
       (format "Nothing was committed: staging %s left the index holding %s, which is not what this commit named"
               (string-join paths ", ")
               (string-join (pearl-tools-git--staged root) ", "))))
     (t
      (let ((run (apply #'pearl-tools-git--git root
                        (list "commit" "-m" message))))
        (if (not (zerop (car run)))
            (progn
              (pearl-tools-git--unstage root paths)
              (pearl-tools-git--failure
               (format "Nothing was committed: %s" (cdr run))))
          (let ((head (cdr (pearl-tools-git--git root "rev-parse" "HEAD"))))
            (append (pearl-contract-tool-result
                     (pearl-tools-git--call)
                     (format "Committed %s: %s"
                             (substring head 0 7)
                             (car (split-string message "\n"))))
                    (list :commit head :epoch epoch :paths paths
                          :gate gate)))))))))

(defun pearl-tools-git-commit (arguments context)
  "Commit the paths ARGUMENTS names, in the root CONTEXT names."
  (let* ((arguments (pearl-tool-normalize-arguments arguments))
         (message (plist-get arguments :message))
         (paths (pearl-tools-git--paths (plist-get arguments :paths)))
         (note (plist-get arguments :gate))
         (gate (and note (pearl-tree-epoch-from-note note)))
         (root (plist-get context :root))
         (epoch (and root (pearl-tree-epoch root)))
         (moved (and gate epoch (pearl-tools-git--moved-since gate paths epoch)))
         (staged (and root (pearl-tools-git--staged root))))
    (cond
     ((null root)
      (pearl-tools-git--failure "Nothing was committed: there is no project root"))
     ((not (and (stringp message) (not (string-empty-p message))))
      (pearl-tools-git--failure "Nothing was committed: a commit needs a message"))
     ((null paths)
      (pearl-tools-git--failure
       "Nothing was committed: paths is required; a commit stages what it names and nothing else"))
     (staged
      (pearl-tools-git--failure
       (format "Nothing was committed: the index already holds %s, which this commit did not name"
               (string-join staged ", "))))
     ((and note (not gate))
      (pearl-tools-git--failure
       "Nothing was committed: the gate names no tree; hand back the note the result carried"))
     (moved
      (pearl-tools-git--failure
       (format "Nothing was committed: %s moved since the gate ran; run the gate again"
               (string-join moved ", "))))
     (t (pearl-tools-git--commit root message paths epoch note)))))

(defun pearl-tools-git-declaration ()
  "Return the declaration the model is shown for a commit."
  (pearl-tool-define
   "commit"
   '(:description "Commit the named paths and nothing else: no staged file, no other session's edit."
     :arguments ((message :required t :description "The commit message, subject first")
                 (paths :required t :description "Paths to stage and commit, one per entry")
                 (gate :description "The gate note a passing result carried, copied verbatim")))
   #'pearl-tools-git-commit))

(defun pearl-tools-git-install (registry)
  "REGISTRY with the commit tool added."
  (append registry (list (pearl-tools-git-declaration))))

(provide 'pearl-tools-git)

;;; pearl-tools-git.el ends here
