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
;;      had written it.  The result carries `:commit' (the new head) and
;;      `:epoch' (the fingerprint it was committed over), and a refused
;;      commit leaves the index as it found it.
;;
;; `pearl-tools-git-install' (REGISTRY) -> registry
;;      Add the tool to the list an extension may replace (ADR 6).

;;; Code:

(require 'cl-lib)
(require 'pearl-contract)
(require 'pearl-proc)
(require 'pearl-tool)

(defun pearl-tools-git--call ()
  "The call a git result carries."
  (list :name "commit"))

(defun pearl-tools-git--failure (message)
  "A git result carrying MESSAGE as the reason it did nothing."
  (pearl-contract-tool-result (pearl-tools-git--call) message t))

(defun pearl-tools-git-commit (arguments context)
  "Commit the paths ARGUMENTS names, in the root CONTEXT names."
  (ignore arguments context)
  (pearl-tools-git--failure "Committing is not written yet"))

(defun pearl-tools-git-declaration ()
  "Return the declaration the model is shown for a commit."
  (pearl-tool-define
   "commit"
   '(:description "Commit the named paths and nothing else: no staged file, no other session's edit."
     :arguments ((message :required t :description "The commit message, subject first")
                 (paths :required t :description "Paths to stage and commit, one per entry")))
   #'pearl-tools-git-commit))

(defun pearl-tools-git-install (registry)
  "REGISTRY with the commit tool added."
  (append registry (list (pearl-tools-git-declaration))))

(provide 'pearl-tools-git)

;;; pearl-tools-git.el ends here
