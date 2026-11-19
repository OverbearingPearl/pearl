;;; pearl-delegate.el --- A delegate is a session of its own  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;; This file is not part of GNU Emacs.

;;; Commentary:

;; A delegate is the one tool that does not answer from the tree it was
;; called in.  It starts a session of its own, with its own file and its
;; own lock (ADR 22), lets it run to a stop, writes what came out to a
;; file, and hands the parent a pointer of at most five hundred
;; characters (ADR 31).
;;
;; The size of that pointer is the point of the arrangement.  A model that
;; reads a report inline pays for the whole report on every later turn; a
;; model that gets a path and a paragraph pays for the paragraph, and asks
;; for the file when it wants the rest.  So nothing here splices a child's
;; transcript into a parent's record, and the parent's projection never
;; changes shape because a child read something.
;;
;; A read-only role is a session file, a loop and a branch, and nothing
;; else.  It writes no project file, so it adds nothing to the undo domain
;; and cannot touch a buffer the user is working in.  A worker would write,
;; and a worker is therefore refused until a parent's plan can expand
;; another session's plan by name; that is the whole reason the roles are
;; ordered this way.
;;
;; A delegate may not ask.  A confirmation from a background loop takes the
;; minibuffer from a user who is typing in it, and a fleet of them takes it
;; repeatedly, so a child either proceeds under its role or fails and says
;; why.

;; Planned API

;; `pearl-delegate-role' (NAME) -> plist
;;      What NAME may do: `:tools' (the tool names the child may run),
;;      `:writes' (non-nil when the child may write the project) and
;;      `:asks' (nil, always, because a delegate may not ask).  A name this
;;      package does not know is not a role: it signals
;;      `pearl-tool-invalid' rather than guessing a tool list, because a
;;      guessed list is a permission nobody granted.

;; `pearl-delegate-declaration' () -> declaration
;;      The `delegate' tool as a model sees it: a role and a task, both
;;      required.  There is no argument for a model, a provider or a
;;      budget: the child runs on the parent's provider (a second transport
;;      is a second set of failures) and a model choosing how many agents
;;      to start is a budget nobody set.

;; `pearl-delegate-start' (ARGUMENTS CONTEXT) -> handle
;;      Start the child and return what it takes to wait for it: `:id',
;;      `:session', `:loop', `:file' and `:store'.  The session is written
;;      as it runs, so a parent killed mid-run leaves a child that can be
;;      read afterwards.

;; `pearl-delegate-pointer' (TEXT) -> string
;;      The first paragraph of TEXT, cut at
;;      `pearl-delegate--pointer-limit' on a word boundary when one is
;;      near.  This is what the parent keeps; the rest stays in the file.

;; `pearl-delegate-run' (ARGUMENTS CONTEXT) -> tool result
;;      The tool body: start, wait for the child to stop, write its text to
;;      `<root>/delegations/<id>.md', and answer with the pointer, the path
;;      and `:delegate' naming the child's session so a plan can find it
;;      later.  A refused role, a missing task or a child that failed are
;;      error results, because a tool never signals at the loop.

;; `pearl-delegate-install' (REGISTRY) -> registry
;;      REGISTRY with the delegate tool added once.

;;; Code:

(require 'cl-lib)
(require 'pearl-contract)
(require 'pearl-loop)
(require 'pearl-session)
(require 'pearl-session-jsonl)
(require 'pearl-tool)
(require 'subr-x)

(defconst pearl-delegate--pointer-limit 500
  "How much of a child's answer the parent keeps.")

(defconst pearl-delegate--roles
  '(("reviewer" :writes nil :tools ("read" "shell"))
    ("researcher" :writes nil :tools ("read" "shell"))
    ("planner" :writes nil :tools ("read" "shell"))
    ("oracle" :writes nil :tools ("read" "shell"))
    ("worker" :writes t :tools ("read" "shell" "edit")))
  "What each role may do.  A role is a permission, not a prompt.")

(defvar pearl-delegate--waiting nil
  "Non-nil while a delegate is being waited for.

A wait blocks inside `accept-process-output', and timers fire there.  A
second delegate started from one of them would open a second child while
the first is half finished, in the same Emacs and against the same root.")

(defvar pearl-delegate--sequence 0
  "How many delegates this Emacs has started, for telling two apart.")

(defun pearl-delegate-role (name)
  "What NAME may do, or a refusal naming the roles that exist."
  (let ((entry (assoc-string (format "%s" name) pearl-delegate--roles)))
    (unless entry
      (signal 'pearl-tool-invalid
              (list (format "no delegate role named %s; the roles are %s"
                            name
                            (mapconcat #'car pearl-delegate--roles ", ")))))
    (append (list :name (car entry)) (cdr entry))))

(defun pearl-delegate--tools (role tools)
  "The list TOOLS narrowed to what ROLE may run."
  (let ((allowed (plist-get role :tools)))
    (seq-filter (lambda (tool) (member (plist-get tool :name) allowed)) tools)))

(defun pearl-delegate-pointer (text)
  "The first paragraph of TEXT, cut at the pointer limit."
  (let* ((limit pearl-delegate--pointer-limit)
         (first (or (car (split-string (or text "") "\n[ \t]*\n")) ""))
         (paragraph (string-trim first)))
    (cond
     ((<= (length paragraph) limit) paragraph)
     (t (let* ((cut (substring paragraph 0 (1- limit)))
               (space (string-match-p "[ \t][^ \t]*\\'" cut)))
          (concat (if space (substring cut 0 space) cut) "…"))))))

(defun pearl-delegate-start (arguments context)
  "Start a child for ARGUMENTS and return the handle that waits on it.
CONTEXT says where the child runs and what it answers through."
  (let* ((arguments (pearl-tool-normalize-arguments arguments))
         (name (plist-get arguments :role))
         (task (plist-get arguments :task))
         (role (pearl-delegate-role name))
         (root (plist-get context :root))
         (parent (plist-get context :loop))
         (provider (or (plist-get context :provider)
                       (plist-get context :provider-function)
                       (and parent (pearl-loop-provider parent))))
         (tools (pearl-delegate--tools
                 role (or (plist-get context :tools)
                          (and parent (pearl-loop-tools parent)))))
         (id (format "%s-%d"
                     (format-time-string "%Y%m%dT%H%M%S")
                     (cl-incf pearl-delegate--sequence)))
         (file (expand-file-name (format "delegations/%s.md" id) root))
         (path (pearl-session-jsonl-path-for root id))
         (store (pearl-session-jsonl-open path :create t :lock t))
         (session (pearl-session-new))
         (collected (list nil))
         (loop nil))
    (setq loop
          (pearl-loop-new
           :provider provider
           :tools tools
           :session session
           :root root
           :guard (lambda () t)
           :emit (lambda (event)
                   (when (eq (plist-get event :kind) 'message-end)
                     (let ((message (plist-get (plist-get event :data) :message)))
                       (when (eq 'assistant (plist-get message :role))
                         (setcar collected
                                 (cons (pearl-contract-text message)
                                       (car collected)))))))))
    (when task
      (pearl-loop-submit loop (pearl-contract-message 'user task)))
    (list :id id :session session :loop loop :file file :store store
          :collected collected)))

(defun pearl-delegate--wait (handle)
  "Wait until HANDLE's child has stopped, and nothing else."
  (let ((loop (plist-get handle :loop))
        (deadline (+ (float-time) 300)))
    (while (and (pearl-loop-busy-p loop)
                (< (float-time) deadline))
      (accept-process-output nil 0.01))))

(defun pearl-delegate--text (handle)
  "Everything HANDLE's child has said, oldest first."
  (mapconcat #'identity (reverse (car (plist-get handle :collected))) "\n\n"))

(defun pearl-delegate--finish (handle)
  "Write HANDLE's child's answer to its file and close its store."
  (let ((text (pearl-delegate--text handle))
        (file (plist-get handle :file)))
    (make-directory (file-name-directory file) t)
    (with-temp-file file
      (insert (or text "")))
    (condition-case nil (pearl-session-jsonl-close (plist-get handle :store))
      (error nil))
    text))

(defun pearl-delegate-run (arguments context)
  "Run one delegate described by ARGUMENTS and answered by CONTEXT."
  (condition-case error
      (let* ((arguments (pearl-tool-normalize-arguments arguments))
             (name (plist-get arguments :role))
             (role (pearl-delegate-role name)))
        (unless (plist-get arguments :task)
          (signal 'pearl-tool-invalid
                  (list "a delegate needs a task to do")))
        (when (plist-get role :writes)
          (signal 'pearl-tool-invalid
                  (list (format "%s may write, and a write is not in a plan yet"
                                name))))
        (when pearl-delegate--waiting
          (signal 'pearl-tool-invalid
                  (list "a delegate is already waiting; finish it first")))
        (let* ((handle (pearl-delegate-start arguments context))
               (text (let ((pearl-delegate--waiting t))
                       (pearl-delegate--wait handle)
                       (pearl-delegate--finish handle)))
               (pointer (pearl-delegate-pointer text))
               (result (pearl-contract-tool-result
                        (list :name "delegate")
                        (format "%s\n\nFull result: %s"
                                pointer (plist-get handle :file))
                        nil)))
          (plist-put result :delegate (plist-get handle :id))
          (plist-put result :file (plist-get handle :file))))
    ((quit) (pearl-tool-error-result nil "the user escaped"))
    (error (pearl-tool-error-result nil (error-message-string error)))))

(defun pearl-delegate-declaration ()
  "The tool a model sees when it wants a second opinion."
  (list :description "Have a delegate read something and report back."
        :arguments '((role :required t
                           :description "reviewer, researcher, planner or oracle")
                     (task :required t
                           :description "What to read or work out."))))

(defun pearl-delegate-install (registry)
  "REGISTRY with the delegate tool added once."
  (if (seq-some (lambda (tool) (equal "delegate" (plist-get tool :name))) registry)
      registry
    (append registry
            (list (pearl-tool-define "delegate"
                                     (pearl-delegate-declaration)
                                     #'pearl-delegate-run)))))

(provide 'pearl-delegate)

;;; pearl-delegate.el ends here
