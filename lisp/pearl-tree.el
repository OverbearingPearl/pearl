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
;; `pearl-tree-environment' () -> list of strings
;;      `process-environment' with the variables that name another
;;      repository removed.  `git' reads GIT_DIR before it reads `-C', so a
;;      run inside a hook or beside a nested checkout would describe the
;;      caller's tree while claiming to describe this one.
;;
;; `pearl-tree-epoch-note' (EPOCH) -> string
;;      EPOCH as one line-per-path note, so it can travel through a tool
;;      argument: the head, then a path, its size and its modification time
;;      per line.  A model copies it back verbatim; nothing about the shape
;;      has to be understood to carry it.
;;
;; `pearl-tree-epoch-from-note' (NOTE) -> epoch or nil
;;      The epoch NOTE was written from, or nil when it names no tree.  The
;;      inverse of `pearl-tree-epoch-note', and the only thing a commit that
;;      cites a gate needs.
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
  (let ((process-environment (pearl-tree-environment)))
    (with-temp-buffer
      (when (zerop (apply #'call-process "git" nil t nil
                          "-C" root "--no-optional-locks" arguments))
        (buffer-string)))))

(defconst pearl-tree--naming-variables
  '("GIT_DIR" "GIT_WORK_TREE" "GIT_INDEX_FILE" "GIT_PREFIX" "GIT_OBJECT_DIRECTORY"
    "GIT_ALTERNATE_OBJECT_DIRECTORIES" "GIT_COMMON_DIR" "GIT_NAMESPACE")
  "The environment variables that make git describe some other tree.")

(defun pearl-tree-environment ()
  "Return `process-environment' with the variables that name another tree gone.
`git' reads GIT_DIR before it reads `-C', so without this a run inside a
hook, or beside a nested checkout, describes the caller's repository while
claiming to describe the root it was given."
  (seq-remove (lambda (entry)
                (member (car (split-string entry "="))
                        pearl-tree--naming-variables))
              process-environment))

(defun pearl-tree--hash (root path)
  "Return the hash of PATH's text under ROOT, or nil when it is gone.
Two sessions editing different regions of one file produce different text,
and the hash is what says so without reading both again."
  (let ((file (expand-file-name path root)))
    (when (file-regular-p file)
      (with-temp-buffer
        (insert-file-contents-literally file)
        (secure-hash 'sha256 (current-buffer))))))

(defun pearl-tree--entries (root status)
  "Return ((PATH SIZE MTIME HASH) ...) for the paths STATUS reports under ROOT.
The stamp is what makes an in-place rewrite visible: git's line for a
modified file is the same before and after, while the file it describes
is not.  The hash is what lets a later reader ask the narrower question the
stamp cannot answer: is this the same text?"
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
                      (and attributes (file-attribute-modification-time attributes))
                      (pearl-tree--hash root path))
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

(defun pearl-tree-epoch-note (epoch)
  "Return EPOCH as a note a tool argument can carry."
  (when epoch
    (string-join
     ;; The head is "-" when there is none: an empty first line would be
     ;; dropped by `split-string' and the paths would read as the head.
     (append (list (let ((head (plist-get epoch :head)))
                     (if (string-empty-p (or head "")) "-" head)))
             (mapcar (lambda (entry)
                       (format "%s\t%s\t%S\t%s" (nth 0 entry) (nth 1 entry)
                               (nth 2 entry) (or (nth 3 entry) "-")))
                     (plist-get epoch :changed)))
     "\n")))

(defun pearl-tree-epoch-from-note (note)
  "Return the epoch NOTE was written from, or nil.
Nil is the answer for anything that is not a note: a commit that cites a
tree it cannot read is a commit that has not been proved."
  (when (and (stringp note) (not (string-empty-p note)))
    (let ((lines (split-string note "\n" t)))
      (when (and lines (not (string-empty-p (car lines))))
        (list :head (if (equal "-" (car lines)) "" (car lines))
              :changed
              (mapcar (lambda (line)
                        (let ((parts (split-string line "\t")))
                          (when (= 4 (length parts))
                            (list (nth 0 parts)
                                  (string-to-number (nth 1 parts))
                                  (car (read-from-string (nth 2 parts)))
                                  (unless (equal "-" (nth 3 parts)) (nth 3 parts))))))
                      (cdr lines)))))))

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
