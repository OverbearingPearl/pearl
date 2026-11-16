;;; pearl-support-tests.el --- load path and the planned API  -*- lexical-binding: t; -*-

;;; Commentary:

;; Declares every function the stories call before the module that answers it
;; exists, so a story fails with `not implemented' rather than a compiler
;; warning.  The suite puts `vendor/' on the load path for lint and for tests.

;;; Code:

(let* ((file (or load-file-name buffer-file-name))
       (here (file-name-directory file)))
  (add-to-list 'load-path (expand-file-name "../vendor/ert-gwt" here)))

(require 'ert-gwt)

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
(pearl-support--stub pearl-tool-declarations)
(pearl-support--stub pearl-tool-validate)
(pearl-support--stub pearl-tool-run)
(pearl-support--stub pearl-tool-error-result)
(pearl-support--stub pearl-tool-truncate)

;; provider
(pearl-support--stub pearl-provider-request)
(pearl-support--stub pearl-provider-cancel)
(pearl-support--stub pearl-provider-fake)
(pearl-support--stub pearl-provider-p)

;; provider-sse
(pearl-support--stub pearl-provider-sse)
(pearl-support--stub pearl-provider-sse-parse-chunk)
(pearl-support--stub pearl-provider-sse-parse-event)

;; proc
(pearl-support--stub pearl-proc-run)
(pearl-support--stub pearl-proc-check)

;; session-jsonl
(pearl-support--stub pearl-session-jsonl-open)
(pearl-support--stub pearl-session-jsonl-append)
(pearl-support--stub pearl-session-jsonl-read)
(pearl-support--stub pearl-session-jsonl-close)

;; console
(pearl-support--stub pearl-console-open)
(pearl-support--stub pearl-console-send)
(pearl-support--stub pearl-console-insert-event)
(pearl-support--stub pearl-console-status)
(pearl-support--stub pearl-console-abort)
(pearl-support--stub pearl-console-redraw)

;; review
(pearl-support--stub pearl-review-install)
(pearl-support--stub pearl-review-parse)
(pearl-support--stub pearl-review-fold-p)
(pearl-support--stub pearl-review-locate)

;; tools
(pearl-support--stub pearl-tools-read-run)
(pearl-support--stub pearl-tools-read-install)
(pearl-support--stub pearl-tools-edit-run)
(pearl-support--stub pearl-tools-edit-apply)
(pearl-support--stub pearl-tools-edit-install)
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

(provide 'pearl-support-tests)

;;; pearl-support-tests.el ends here
