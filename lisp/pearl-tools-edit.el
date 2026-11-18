;;; pearl-tools-edit.el --- the edit tool  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The one write path that goes through a buffer, and therefore the one that
;; the user's `C-/` takes back.  An edit is a list of disjoint replacements,
;; each anchored by the exact text it removes; a stale anchor is a loud error
;; and a round trip, which is the price ADR 4 accepted for the proof that the
;; model read the file.
;;
;; The whole call is one change group: accepted and amalgamated when every
;; replacement lands, cancelled when one does not, so a half-applied edit is
;; not a state the user can reach.
;;
;; Planned API
;;
;; `pearl-tools-edit-declaration' -> tool declaration
;;      `edits[]', each with `oldText' and `newText', matching Pi's schema
;;      (ADR 8): unique match, no overlaps, several edits per call.
;;
;; `pearl-tools-edit-run' (ARGUMENTS CONTEXT) -> tool result
;;      Locate every anchor before writing anything; refuse the whole call if
;;      one is missing or ambiguous, naming the anchor that failed.  Returns
;;      the file, the replacements applied and, for each, the line it landed
;;      on, so the console can report what changed and where.  A buffer that
;;      was already dirty is not saved, and the result carries `:unsaved'
;;      saying so.
;;
;;      CONTEXT supplies the project root, and a resolved path outside it is
;;      refused by name as an error result, as is a call with no root at all:
;;      ADR 16's rule is that a write is proved only where the turn's snapshot
;;      can take it back, and the shell is not the only writer that owes it.
;;      The refusal is a result rather than a signal for the same reason a
;;      stale anchor is, so the model can say what it meant instead of the
;;      turn dying.
;;
;;      The file on disk is the truth at apply time and the buffer is a cache
;;      (ADR 23).  Re-read the file before locating an anchor: when it differs
;;      from what the buffer holds and the buffer is unmodified, refresh the
;;      buffer from disk and locate the anchors there, so a change to another
;;      part of the file survives an edit that never saw it.  When an anchor is
;;      missing or ambiguous in that text, refuse the whole call, name the
;;      anchor, and say the file changed: a round trip is the price, a merged
;;      guess is not.  When the buffer is modified and the file changed too,
;;      refuse with `:unsaved' and `:changed-on-disk' and write nothing -
;;      both are somebody's work and there is no principled winner.  A save
;;      never overwrites a file that moved.
;;
;; `pearl-tools-edit-apply' (BUFFER EDITS) -> (APPLIED . PATCH)
;;      Open a change group through `pearl-undo-group-prepare', apply each
;;      replacement, and accept it on success or cancel it on failure; return
;;      the number applied and the patch the entry will carry.  This is the
;;      only caller of the undo change-group API in the package, which is what
;;      makes the promise auditable: one call site, one group, one undo step.
;;
;; `pearl-tools-edit-unsaved-p' (BUFFER) -> boolean
;;      Whether the buffer was modified before the edit; a buffer that was
;;      already dirty is not saved by the tool, and the result says so.
;;
;; `pearl-tools-edit-install' (REGISTRY) -> registry
;;      Add the tool to the list an extension may replace (ADR 6).

;;; Code:

(require 'cl-lib)
(require 'seq)
(require 'pearl-contract)
(require 'pearl-tool)
(require 'pearl-undo)

(defun pearl-tools-edit--call ()
  "The call an edit result carries."
  (list :name "edit"))

(defun pearl-tools-edit--failed (message &rest extra)
  "An edit result carrying MESSAGE, and EXTRA as further keys."
  (append (plist-put (pearl-contract-tool-result
                      (pearl-tools-edit--call) message t)
                     :error message)
          extra))

(defun pearl-tools-edit--succeeded (content patch &rest extra)
  "An edit result carrying CONTENT, PATCH and EXTRA."
  (append (pearl-contract-tool-result (pearl-tools-edit--call) content)
          (list :patch patch)
          extra))

(defun pearl-tools-edit--path (arguments context)
  "The file ARGUMENTS name, resolved against the root in CONTEXT."
  (let ((path (plist-get arguments :path))
        (root (plist-get context :root)))
    (cond
     ((null root) (cons nil "No project root: a write has to be one a snapshot can take back"))
     ((or (null path) (equal "" path)) (cons nil "No path given"))
     (t (let ((full (expand-file-name path root)))
          (if (file-in-directory-p full (expand-file-name root))
              (cons full nil)
            (cons nil (format "%s is outside the project root, so nothing would take it back"
                              full))))))))

(defun pearl-tools-edit-unsaved-p (buffer)
  "Non-nil when BUFFER carries work its user has not saved."
  (and (buffer-live-p buffer) (buffer-modified-p buffer)))

(defun pearl-tools-edit--position (text edit)
  "Where EDIT's `:old-text' sits in TEXT, or a description of its failure."
  (let ((old (plist-get edit :old-text)))
    (cond
     ((or (null old) (equal "" old)) (cons nil "an edit with an empty old-text"))
     (t (let ((first (string-search old text)))
          (cond
           ((null first) (cons nil (format "%S" old)))
           ((string-search old text (1+ first)) (cons nil (format "%S" old)))
           (t (cons first nil))))))))

(defun pearl-tools-edit--locate (text edits)
  "Return the positions of EDITS in TEXT, or a failure naming one."
  (let ((found nil)
        (failure nil))
    (dolist (edit edits)
      (if failure
          nil
        (let ((position (pearl-tools-edit--position text edit)))
          (if (cdr position)
              (setq failure (format "Cannot place %s: it is not there once, or not there at all"
                                    (cdr position)))
            (push (cons (car position) edit) found)))))
    (when (and (null failure) found)
      (setq found (sort found (lambda (a b) (< (car a) (car b)))))
      (let ((previous nil))
        (dolist (each found)
          (when (and previous
                     (< (car each)
                        (+ (car previous) (length (plist-get (cdr previous) :old-text)))))
            (setq failure (format "Cannot place %S: it overlaps another edit"
                                  (plist-get (cdr each) :old-text))))
          (setq previous each))))
    (if failure (cons nil failure) (cons found nil))))

(defun pearl-tools-edit--replace (found)
  "Replace what FOUND records, last position first, and return the count."
  (let ((count 0))
    (dolist (each (reverse found))
      (goto-char (1+ (car each)))
      (let ((offset (point)))
        (delete-region offset (+ offset (length (plist-get (cdr each) :old-text))))
        (insert (or (plist-get (cdr each) :new-text) ""))
        (setq count (1+ count))))
    count))

(defun pearl-tools-edit-apply (buffer edits)
  "Apply EDITS to BUFFER as one undo step, returning (APPLIED . PATCH)."
  (let* ((handle (pearl-undo-group-prepare buffer))
         (text (with-current-buffer buffer (buffer-string)))
         (located (pearl-tools-edit--locate text edits))
         (failure (cdr located))
         (file (buffer-file-name buffer))
         (applied 0))
    (if failure
        (progn (pearl-undo-group-cancel handle)
               (signal 'pearl-tools-edit-conflict (list failure)))
      (condition-case err
          (with-current-buffer buffer
            (save-excursion
              (setq applied (pearl-tools-edit--replace (car located)))))
        (error (pearl-undo-group-cancel handle)
               (signal (car err) (cdr err))))
      (pearl-undo-group-accept handle)
      (cons applied (list :tool "edit"
                          :file file
                          :old-text (plist-get (car edits) :old-text)
                          :new-text (plist-get (car edits) :new-text))))))

(defun pearl-tools-edit-run (arguments context)
  "Edit the file ARGUMENTS name, in the project CONTEXT names."
  (let* ((arguments (pearl-tool-normalize-arguments arguments))
         (resolved (pearl-tools-edit--path arguments context))
         (path (car resolved)))
    (if (cdr resolved)
        (pearl-tools-edit--failed (cdr resolved))
      (cond
       ((not (file-exists-p path))
        (pearl-tools-edit--failed (format "No such file: %s" path)))
       ((not (listp (plist-get arguments :edits)))
        (pearl-tools-edit--failed "No edits given"))
       (t
        (let* ((buffer (or (find-buffer-visiting path)
                           (find-file-noselect path t)))
               (unsaved (pearl-tools-edit-unsaved-p buffer))
               (moved (with-current-buffer buffer
                        (not (verify-visited-file-modtime buffer)))))
          (if (and unsaved moved)
              (pearl-tools-edit--failed
               (format "%s changed on disk while your buffer holds unsaved \
changes; nothing was written"
                       (file-name-nondirectory path))
               :unsaved t :changed-on-disk t)
            (when moved
              ;; The file is the truth and the buffer is a cache: a change to
              ;; another part of the file survives an edit that never saw it.
              (with-current-buffer buffer (revert-buffer t t t)))
            (let* ((edits (plist-get arguments :edits))
                   (text (with-current-buffer buffer (buffer-string)))
                   (located (pearl-tools-edit--locate text edits)))
              (if (cdr located)
                  (pearl-tools-edit--failed
                   (format "Cannot apply the edit: %s%s" (cdr located)
                           (if moved
                               ", and the file changed on disk since you read it"
                             "")))
                (let* ((applied (pearl-tools-edit-apply buffer edits))
                       (count (car applied)))
                  (unless unsaved
                    (with-current-buffer buffer (save-buffer)))
                  (let ((result (pearl-tools-edit--succeeded
                                 (format "Replaced %d occurrence%s in %s"
                                         count
                                         (if (= count 1) "" "s")
                                         (file-relative-name path (plist-get context :root)))
                                 (cdr applied))))
                    (if unsaved (append result (list :unsaved t)) result))))))))))))

(defun pearl-tools-edit-declaration ()
  "The declaration of the edit tool."
  (pearl-tool-define
   "edit"
   '(:description "Replace text in a file, once or several times, as one undo step."
     :arguments ((path :required t :description "File to edit")
                 (edits :required t
                        :description "Replacements, each an oldText and a newText")))
   #'pearl-tools-edit-run))

(defun pearl-tools-edit-install (registry)
  "Add the edit tool to REGISTRY, once."
  (if (seq-some (lambda (tool) (equal "edit" (plist-get tool :name))) registry)
      registry
    (append registry (list (pearl-tools-edit-declaration)))))

(provide 'pearl-tools-edit)

;;; pearl-tools-edit.el ends here
