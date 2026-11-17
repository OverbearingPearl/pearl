;;; pearl-support-tests.el --- load path, helpers and the planned API  -*- lexical-binding: t; -*-

;;; Commentary:

;; Declares every function the stories call before the module that answers it
;; exists, and the few helpers every story shares: waiting on the event loop,
;; temp roots, and a local TCP server the SSE stories stream from.

;;; Code:

(let* ((file (or load-file-name buffer-file-name))
       (here (file-name-directory file)))
  (add-to-list 'load-path (expand-file-name "../vendor/ert-gwt" here)))

(require 'ert-gwt)

(defun pearl-support-wait (predicate &optional seconds)
  "Pump the event loop until PREDICATE is non-nil or SECONDS elapse.
SECONDS defaults to 5.  Return the value of PREDICATE, so a story can
assert on the wait itself: a nil return means the run never finished."
  (let ((deadline (+ (float-time) (or seconds 5))))
    (while (and (not (funcall predicate))
                (< (float-time) deadline))
      (accept-process-output nil 0.01))
    (funcall predicate)))

(defun pearl-support-temp-root ()
  "Return a fresh temp directory, registered for the story's cleanup."
  (let ((root (make-temp-file "pearl-root" t)))
    (push root ert-gwt--tracked-files)
    root))

(defun pearl-support-git-root ()
  "Return a fresh temp directory initialized as a git repository.
The snapshot layer is a dangling commit, so a story about snapshots
needs a repository; `pearl-undo-snapshot' returns nil outside one."
  (let* ((root (pearl-support-temp-root))
         (default-directory root))
    (call-process "git" nil nil nil "init" "-q")
    (call-process "git" nil nil nil "config" "user.email" "story@example.com")
    (call-process "git" nil nil nil "config" "user.name" "Story")
    root))

(defun pearl-support-tcp-server (on-open)
  "Listen on a local port and call ON-OPEN with each accepted client.
Return the server process; the port is (process-contact SERVER :service).
The story closes it with `pearl-support-tcp-close' in its cleanup."
  (make-network-process
   :name "pearl-story-server" :server t :host "127.0.0.1" :service 0
   :family 'ipv4 :noquery t
   :sentinel (lambda (client _event)
               (when (eq (process-status client) 'open)
                 (process-put client 'pearl-story-server t)
                 (funcall on-open client)))))

(defun pearl-support-tcp-close (server)
  "Delete SERVER and every client connection it accepted."
  (dolist (proc (process-list))
    (when (and (process-live-p proc)
               (or (eq proc server)
                   (process-get proc 'pearl-story-server)))
      (delete-process proc))))

(defun pearl-support-sse-frame (payload)
  "Return PAYLOAD as one complete SSE data frame."
  (concat "data: " payload "\n\n"))

;; Variables the modules will own; declared here so a story can bind them
;; before the module lands.  A defvar without a value never overrides the
;; module's own definition once it exists.
(defvar pearl-tool-registry nil
  "The default tool list an extension rebinds (ADR 6).")
(defvar pearl-console-provider nil
  "The provider a console wires its loop over; a story binds a fake.")

(defmacro pearl-support--stub (name)
  "Make NAME a function that reports it is not implemented.
Does nothing when NAME is already defined, so a landed module wins."
  `(unless (fboundp ',name)
     (defalias ',name
       (lambda (&rest _)
         (error "PEARL: %s is not implemented yet" ',name)))))

;; contract
(pearl-support--stub pearl-contract-message)
(pearl-support--stub pearl-contract-message-p)
(pearl-support--stub pearl-contract-text)
(pearl-support--stub pearl-contract-tool-call)
(pearl-support--stub pearl-contract-tool-result)
(pearl-support--stub pearl-contract-tool-declaration)
(pearl-support--stub pearl-contract-to-wire)
(pearl-support--stub pearl-contract-from-wire)
(pearl-support--stub pearl-contract-entry)

;; event
(pearl-support--stub pearl-event-kinds)
(pearl-support--stub pearl-event-emitter)
(pearl-support--stub pearl-event-new)
(pearl-support--stub pearl-event-subscribe)
(pearl-support--stub pearl-event-emit)
(pearl-support--stub pearl-event-render-hint)

;; loop
(pearl-support--stub pearl-loop-new)
(pearl-support--stub pearl-loop-submit)
(pearl-support--stub pearl-loop-steer)
(pearl-support--stub pearl-loop-follow-up)
(pearl-support--stub pearl-loop-abort)
(pearl-support--stub pearl-loop-busy-p)
(pearl-support--stub pearl-loop-queued)
(pearl-support--stub pearl-loop-subscribe)
(pearl-support--stub pearl-loop-set-tools)

;; session
(pearl-support--stub pearl-session-new)
(pearl-support--stub pearl-session-append)
(pearl-support--stub pearl-session-entries)
(pearl-support--stub pearl-session-head)
(pearl-support--stub pearl-session-parent)
(pearl-support--stub pearl-session-set-head)
(pearl-support--stub pearl-session-branch)
(pearl-support--stub pearl-session-projection)
(pearl-support--stub pearl-session-patch)
(pearl-support--stub pearl-session-rollback-plan)
(pearl-support--stub pearl-session-conflict-p)

;; tool
(pearl-support--stub pearl-tool-define)
(pearl-support--stub pearl-tool-list)
(pearl-support--stub pearl-tool-declarations)
(pearl-support--stub pearl-tool-validate)
(pearl-support--stub pearl-tool-run)
(pearl-support--stub pearl-tool-error-result)
(pearl-support--stub pearl-tool-truncate)

;; provider
(pearl-support--stub pearl-provider-request)
(pearl-support--stub pearl-provider-cancel)
(pearl-support--stub pearl-provider-fake)
(pearl-support--stub pearl-provider-fake-requests)
(pearl-support--stub pearl-provider-p)

;; provider-sse
(pearl-support--stub pearl-provider-sse)
(pearl-support--stub pearl-provider-sse-headers)
(pearl-support--stub pearl-provider-sse-request-body)
(pearl-support--stub pearl-provider-sse-parse-chunk)
(pearl-support--stub pearl-provider-sse-parse-event)
(pearl-support--stub pearl-provider-sse-tool-call-delta)
(pearl-support--stub pearl-provider-sse-cancel)

;; proc
(pearl-support--stub pearl-proc-run)
(pearl-support--stub pearl-proc-check)
(pearl-support--stub pearl-proc-kill)

;; session-jsonl
(pearl-support--stub pearl-session-jsonl-open)
(pearl-support--stub pearl-session-jsonl-append)
(pearl-support--stub pearl-session-jsonl-read)
(pearl-support--stub pearl-session-jsonl-close)
(pearl-support--stub pearl-session-jsonl-path-for)

;; console
(pearl-support--stub pearl-console-open)
(pearl-support--stub pearl-console-wire)
(pearl-support--stub pearl-console-mode)
(pearl-support--stub pearl-console-send)
(pearl-support--stub pearl-console-insert-event)
(pearl-support--stub pearl-console-status)
(pearl-support--stub pearl-console-abort)
(pearl-support--stub pearl-console-redraw)
(pearl-support--stub pearl-console-rollback)
(pearl-support--stub pearl-console-close)

;; review
(pearl-support--stub pearl-review-install)
(pearl-support--stub pearl-review-parse)
(pearl-support--stub pearl-review-fold-p)
(pearl-support--stub pearl-review-locate)
(pearl-support--stub pearl-review-xref-backend)

;; tools
(pearl-support--stub pearl-tools-read-declaration)
(pearl-support--stub pearl-tools-read-run)
(pearl-support--stub pearl-tools-read-install)
(pearl-support--stub pearl-tools-edit-declaration)
(pearl-support--stub pearl-tools-edit-run)
(pearl-support--stub pearl-tools-edit-apply)
(pearl-support--stub pearl-tools-edit-unsaved-p)
(pearl-support--stub pearl-tools-edit-install)
(pearl-support--stub pearl-tools-shell-declaration)
(pearl-support--stub pearl-tools-shell-run)
(pearl-support--stub pearl-tools-shell-confirm)
(pearl-support--stub pearl-tools-shell-scope)
(pearl-support--stub pearl-tools-shell-install)

;; undo
(pearl-support--stub pearl-undo-group-prepare)
(pearl-support--stub pearl-undo-group-accept)
(pearl-support--stub pearl-undo-group-cancel)
(pearl-support--stub pearl-undo-snapshot)
(pearl-support--stub pearl-undo-restore)
(pearl-support--stub pearl-undo-conflict-p)
(pearl-support--stub pearl-undo-attach)
(pearl-support--stub pearl-undo-turn-begin)
(pearl-support--stub pearl-undo-turn-end)
(pearl-support--stub pearl-undo-turn-snapshot)
(pearl-support--stub pearl-undo-latest)

(provide 'pearl-support-tests)

;;; pearl-support-tests.el ends here
