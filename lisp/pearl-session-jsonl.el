;;; pearl-session-jsonl.el --- append-only storage for a session  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; One JSON object per line, appended and never rewritten.  The format is the
;; adapter's business and nothing above it may assume a file: an entry in, a
;; line out, and the ability to read every line back into entries.
;;
;; Append-only is not an optimisation.  A session that can be rewritten is a
;; session whose history can disagree with what the user watched happen, and
;; the promise that a change can be taken back after a restart depends on the
;; record being the same one that was written.
;;
;; Append-only assumes one writer, and two Emacsen on one project are easy
;; (ADR 22).  A writer therefore takes a lock beside the file and the second
;; one is refused by name instead of producing a conversation that answers
;; itself; a reader does not lock, because reading a file somebody else is
;; appending to is a read of a valid prefix.
;;
;; Planned API
;;
;; `pearl-session-jsonl-open' (PATH &key CREATE LOCK) -> store
;;      A store is an opaque struct holding the path, the open handle and the
;;      lock it took, if any.  Signals `pearl-session-jsonl-corrupt' on a line
;;      that does not parse, naming the line number: a half-written tail is a
;;      fact to report, not something to skip quietly.
;;
;;      LOCK means "I intend to write".  The lock is a directory `<PATH>.lock'
;;      holding an `owner' file with the pid that took it, because creating a
;;      directory is the one atomic create every platform offers and a
;;      check-then-write on a plain file is a race with a smaller name.  A live
;;      holder signals `pearl-session-locked' naming the path and the pid; a
;;      holder whose pid is gone is stale, and the lock is removed and taken
;;      over once.  Where the platform cannot say whether a pid lives, the open
;;      refuses and names the lock, so the user removes an artifact rather than
;;      guesses.
;;
;; `pearl-session-jsonl-append' (STORE ENTRY) -> entry
;;      Write one line and flush.  Flushing per entry is what makes a killed
;;      Emacs lose at most the entry being written.
;;
;; `pearl-session-jsonl-read' (STORE) -> list of entries
;;      Every entry in file order.  The caller rebuilds the tree by `parentId';
;;      this module does not know what a parent means.
;;
;; `pearl-session-jsonl-close' (STORE) -> nil
;;      Close the handle and release the lock this store took.  Idempotent,
;;      because shutdown runs it twice, and idempotent about the lock too: a
;;      close that finds nothing to release is not an error.
;;
;; `pearl-session-jsonl-path-for' (ROOT SESSION-ID) -> path
;;      Where a session lives under a project root.  The layout is an adapter
;;      decision and stays here.

;;; Code:

(require 'json)
(require 'subr-x)

(define-error 'pearl-session-jsonl-corrupt "A session file has a line that does not parse")
(define-error 'pearl-session-locked "Another Emacs is writing this session")

(cl-defstruct (pearl-session-jsonl--store (:constructor pearl-session-jsonl--make))
  path lock)

(defun pearl-session-jsonl-path-for (root session-id)
  "Where the session SESSION-ID lives under ROOT."
  (expand-file-name (format "sessions/%s.jsonl" session-id)
                    (file-name-as-directory root)))

(defun pearl-session-jsonl--holder (lock)
  "The pid the lock at LOCK names, or nil."
  (let ((owner (expand-file-name "owner" lock)))
    (when (file-exists-p owner)
      (string-to-number
       (string-trim (with-temp-buffer
                      (insert-file-contents owner)
                      (buffer-string)))))))

(defun pearl-session-jsonl--alive-p (pid)
  "Non-nil when PID names a process this machine can see."
  (and pid (> pid 0) (process-attributes pid) t))

(defun pearl-session-jsonl--take-lock (path)
  "Take the lock beside PATH, or signal `pearl-session-locked'."
  (let ((lock (concat path ".lock")))
    (condition-case nil
        (make-directory lock)
      (file-already-exists
       (let ((holder (pearl-session-jsonl--holder lock)))
         (if (pearl-session-jsonl--alive-p holder)
             (signal 'pearl-session-locked
                     (list :path path :pid holder))
           ;; One takeover, for a holder that is gone: a killed Emacs costs
           ;; the next one this, and not an afternoon.
           (pearl-session-jsonl--remove-lock lock)
           (make-directory lock))
         nil)))
    (with-temp-file (expand-file-name "owner" lock)
      (insert (number-to-string (emacs-pid))))
    lock))

(defun pearl-session-jsonl--remove-lock (lock)
  "Remove LOCK, or leave it when it is already gone."
  (condition-case nil (delete-directory lock t) (file-missing nil) (file-error nil)))

(defun pearl-session-jsonl-open (path &rest keys)
  "Open the record at PATH, creating it for KEYS `:create' and `:lock'."
  (let ((create (plist-get keys :create))
        (lock (plist-get keys :lock)))
    (when create
      (make-directory (file-name-directory path) t)
      (unless (file-exists-p path)
        (with-temp-file path nil)))
    (unless (file-exists-p path)
      (signal 'file-missing (list "Opening session" path)))
    (pearl-session-jsonl--make
     :path path
     :lock (when lock (pearl-session-jsonl--take-lock path)))))

(defun pearl-session-jsonl--encode (value)
  "VALUE as something JSON can hold."
  (cond
   ((null value) :null)
   ((or (eq value t) (numberp value) (stringp value)) value)
   ((eq value :null) :null)
   ((symbolp value) (list :symbol (symbol-name value)))
   ((and (consp value) (keywordp (car value)))
    (let (out)
      (while (consp value)
        (push (intern (substring (symbol-name (car value)) 1)) out)
        (push (pearl-session-jsonl--encode (cadr value)) out)
        (setq value (cddr value)))
      (nreverse out)))
   ((vectorp value) (vconcat (mapcar #'pearl-session-jsonl--encode value)))
   ((consp value) (vconcat (mapcar #'pearl-session-jsonl--encode value)))
   (t (format "%s" value))))

(defun pearl-session-jsonl--decode (value)
  "The shape of VALUE inside the record."
  (cond
   ((null value) nil)
   ((vectorp value) (mapcar #'pearl-session-jsonl--decode (append value nil)))
   ((and (consp value) (null (cddr value)) (eq (car value) :symbol))
    (intern (cadr value)))
   ((consp value)
    (let (out)
      (while (consp value)
        (push (car value) out)
        (push (pearl-session-jsonl--decode (cadr value)) out)
        (setq value (cddr value)))
      (nreverse out)))
   (t value)))

(defun pearl-session-jsonl--line (entry)
  "ENTRY as one line of JSON."
  (concat (json-serialize (pearl-session-jsonl--encode entry)) "\n"))

(defun pearl-session-jsonl-append (store entry)
  "Write ENTRY to STORE and return it."
  (let ((coding-system-for-write 'utf-8-unix))
    (write-region (pearl-session-jsonl--line entry) nil
                  (pearl-session-jsonl--store-path store) t 'silent))
  entry)

(defun pearl-session-jsonl--parse (string number)
  "The entry in STRING, or a corrupt signal naming NUMBER."
  (condition-case error
      (pearl-session-jsonl--decode
       (json-parse-string string :object-type 'plist
                                 :null-object nil :false-object nil))
    (json-error
     (signal 'pearl-session-jsonl-corrupt
             (list (format "line %d does not parse: %s"
                           number (error-message-string error))
                   number)))))

(defun pearl-session-jsonl-read (store)
  "Every entry in STORE, in file order."
  (let ((number 0)
        (entries nil))
    (when (file-exists-p (pearl-session-jsonl--store-path store))
      (with-temp-buffer
        (insert-file-contents (pearl-session-jsonl--store-path store))
        (goto-char (point-min))
        (while (not (eobp))
          (setq number (1+ number))
          (let ((line (buffer-substring-no-properties
                       (line-beginning-position) (line-end-position))))
            (unless (string-empty-p (string-trim line))
              (push (pearl-session-jsonl--parse line number) entries)))
          (forward-line 1))))
    (nreverse entries)))

(defun pearl-session-jsonl-close (store)
  "Close STORE and release the lock it took."
  (when store
    (let ((lock (pearl-session-jsonl--store-lock store)))
      (when lock
        (pearl-session-jsonl--remove-lock lock)
        (setf (pearl-session-jsonl--store-lock store) nil))))
  nil)

(provide 'pearl-session-jsonl)

;;; pearl-session-jsonl.el ends here
