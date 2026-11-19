;;; pearl-tree.el --- what the working tree looked like  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; A check, an edit and a commit all talk to a directory another session may
;; be writing at the same time, and none of them can say which tree its
;; conclusion is about.  A green check cannot be told from a check that ran
;; while somebody else was mid-edit, and a commit cannot tell its own paths
;; from the ones another session left half-written.
;;
;; This module gives that tree a name: the commit at the head, plus every path
;; git reports as changed, each with its size and modification time.  Two
;; epochs are equal exactly when nothing git can see has moved, so a command
;; that ran between two different epochs can say its verdict is about a tree
;; that is no longer there instead of reporting a stale pass or a stale fail.
;;
;; The fingerprint is deliberately coarse: it answers "did anything move", not
;; "was my change overwritten".  A refusal to write, undo or commit compares
;; the anchored patch (ADR 4), because two sessions may change different
;; regions of one file and both be right; refusing on "this file was touched"
;; would refuse the safe case (ADR 33).

;; Planned API
;;
;; `pearl-tree-epoch' (ROOT) -> epoch or nil
;;      A fingerprint of the working tree at ROOT: `(:head COMMIT
;;      :changed ((PATH SIZE MTIME) ...))'.  Nil when the tree has no
;;      repository or git refused to answer, which is a state the caller
;;      reports rather than reads as "nothing changed".
;;
;; `pearl-tree-drift' (BEFORE AFTER) -> list of paths or nil
;;      The paths whose entry differs between two epochs, sorted.  Nil when
;;      either epoch is nil: a comparison with no name on one end cannot say
;;      the tree held still, and claiming it did is the failure this module
;;      exists to prevent.

;;; Code:

(require 'cl-lib)
(require 'seq)
(require 'subr-x)

(defun pearl-tree--git (root arguments)
  "Return what git prints for ARGUMENTS in ROOT, or nil when it refuses.
`--no-optional-locks' keeps a read from taking the index lock another
session may be holding: a status that waits is a status that hangs the
turn it was asked from."
  (with-temp-buffer
    (when (zerop (apply #'call-process "git" nil t nil
                        "-C" root "--no-optional-locks" arguments))
      (buffer-string))))

(defun pearl-tree--entries (root status)
  "Return ((PATH SIZE MTIME) ...) for the paths STATUS reports under ROOT.
The stamp is what makes an in-place rewrite visible: git's line for a
modified file is the same before and after, while the file it describes
is not."
  (let ((entries '()))
    (dolist (line (split-string status "\n" t))
      (when (> (length line) 3)
        (let* ((tail (substring line 3))
               (path (if (string-match " -> " tail)
                         (substring tail (match-end 0))
                       tail))
               (path (string-trim path "\"" "\""))
               (attributes (file-attributes (expand-file-name path root))))
          (push (list path
                      (and attributes (file-attribute-size attributes))
                      (and attributes (file-attribute-modification-time attributes)))
                entries))))
    (sort (nreverse entries) (lambda (a b) (string< (car a) (car b))))))

(defun pearl-tree-epoch (root)
  "Return a fingerprint of the working tree at ROOT, or nil.
The fingerprint is the commit at the head and every path git reports as
changed, each with its size and modification time.  Nil means the tree
has no repository, or that git refused to answer: an unknown tree is not
a still one, and the caller reports it instead of reading it as quiet."
  (when (and root
             (file-directory-p root)
             (file-exists-p (expand-file-name ".git" root)))
    (when-let* ((status (pearl-tree--git
                        root '("status" "--porcelain=v1"
                               "--untracked-files=all"))))
      (list :head (string-trim
                   (or (pearl-tree--git root
                                        '("rev-parse" "--verify" "-q" "HEAD"))
                       ""))
            :changed (pearl-tree--entries root status)))))

(defun pearl-tree-drift (before after)
  "Return the paths whose entry differs between BEFORE and AFTER, sorted.
Nil when either epoch is nil: a comparison with no name on one end
cannot say the tree held still."
  (when (and before after)
    (let ((old (plist-get before :changed))
          (new (plist-get after :changed)))
      (sort (seq-remove
             (lambda (path)
               (equal (cdr (assoc path old)) (cdr (assoc path new))))
             (seq-union (mapcar #'car old) (mapcar #'car new)))
            #'string<))))

(provide 'pearl-tree)

;;; pearl-tree.el ends here
