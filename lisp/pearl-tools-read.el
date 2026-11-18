;;; pearl-tools-read.el --- the read tool  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; Read a file, or a window of one, and say which window it was.  The offsets
;; are part of the answer, not of the request alone: a model that is told it
;; saw lines 40 to 80 can ask for the next window, and one that is told
;; nothing asks again.
;;
;; A read never fails the turn.  A missing file, a binary file or an offset
;; past the end is a result the model can act on.
;;
;; Planned API
;;
;; `pearl-tools-read-declaration' -> tool declaration
;;      `path', `offset' and `limit', with the same names and defaults as Pi's
;;      read tool (ADR 8), so a model trained on one works with the other.
;;
;; `pearl-tools-read-run' (ARGUMENTS CONTEXT) -> tool result
;;      The returned text names the range it covers and the total line count,
;;      and the tool never signals: a missing file is an error result.
;;      CONTEXT supplies the project root, which is the only place a relative
;;      path is resolved against.
;;
;; `pearl-tools-read-install' (REGISTRY) -> registry
;;      Add the tool to the list an extension may replace (ADR 6), and return
;;      the new list so installation reads as composition.

;;; Code:

(require 'pearl-contract)
(require 'pearl-tool)

(defconst pearl-tools-read-limit 2000
  "How many lines a read returns when the model names no limit.")

(defun pearl-tools-read--call ()
  "The call a read result carries."
  (list :name "read"))

(defun pearl-tools-read--failure (message)
  "A read result carrying MESSAGE."
  (pearl-contract-tool-result (pearl-tools-read--call) message t))

(defun pearl-tools-read--path (arguments context)
  "The path ARGUMENTS name, resolved against CONTEXT."
  (let ((path (alist-get 'path arguments)))
    (if (file-name-absolute-p path)
        path
      (expand-file-name path (plist-get context :root)))))

(defun pearl-tools-read-declaration ()
  "The declaration of the read tool."
  (pearl-tool-define
   "read"
   '(:description "Read a file, or a window of one, naming the lines returned."
     :arguments ((path :required t :description "File to read")
                 (offset :description "First line, counting from 1")
                 (limit :description "How many lines")))
   #'pearl-tools-read-run))

(defun pearl-tools-read-run (arguments context)
  "Read what ARGUMENTS name, resolving a relative path against CONTEXT."
  (let ((path (pearl-tools-read--path arguments context)))
    (cond
     ((not (file-exists-p path)) (pearl-tools-read--failure
                                  (format "No such file: %s" path)))
     ((file-directory-p path) (pearl-tools-read--failure
                               (format "That is a directory: %s" path)))
     ((not (file-readable-p path)) (pearl-tools-read--failure
                                    (format "Not readable: %s" path)))
     (t
      (let ((text (with-temp-buffer
                    (insert-file-contents path)
                    (buffer-string))))
        (if (seq-contains-p text 0)
            ;; A file of undecodable bytes is not one to read out loud: the
            ;; model would reason about mojibake whose shape it cannot see.
            (pearl-tools-read--failure (format "That file is binary: %s" path))
          (let ((lines (split-string text "\n")))
            (when (and (string-suffix-p "\n" text) (< 1 (length lines)))
              (setq lines (butlast lines)))
            (let* ((total (length lines))
               (first (or (alist-get 'offset arguments) 1))
               (count (or (alist-get 'limit arguments) pearl-tools-read-limit))
               (last (min total (+ first (1- count)))))
          (cond
           ((< total 1) (pearl-tools-read--failure
                         (format "That file is empty: %s" path)))
           ((> first total)
            (pearl-tools-read--failure
             (format "%s has %d lines, so line %d is past the end"
                     path total first)))
           (t
            (pearl-contract-tool-result
             (pearl-tools-read--call)
             (format "lines %d-%d of %d\n\n%s"
                     first last total
                     (mapconcat #'identity
                                (seq-subseq lines (1- first) last) "\n")))))))))))))

(defun pearl-tools-read-install (registry)
  "REGISTRY with the read tool added."
  (append registry (list (pearl-tools-read-declaration))))

(provide 'pearl-tools-read)

;;; pearl-tools-read.el ends here
