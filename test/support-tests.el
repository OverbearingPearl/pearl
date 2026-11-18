;;; support-tests.el --- load path, helpers and the planned API  -*- lexical-binding: t; -*-

;;; Commentary:

;; Declares every function the stories call before the module that answers it
;; exists, and the few helpers every story shares: waiting on the event loop,
;; temp roots, and a local TCP server the SSE stories stream from.

;;; Code:

(let* ((file (or load-file-name buffer-file-name))
       (here (file-name-directory file)))
  (add-to-list 'load-path (expand-file-name "../vendor/ert-gwt" here)))

(require 'ert-gwt)

(defun support-tests-wait (predicate &optional seconds)
  "Pump the event loop until PREDICATE is non-nil or SECONDS elapse.
SECONDS defaults to 5.  Return the value of PREDICATE, so a story can
assert on the wait itself: a nil return means the run never finished."
  (let ((deadline (+ (float-time) (or seconds 5))))
    (while (and (not (funcall predicate))
                (< (float-time) deadline))
      (accept-process-output nil 0.01))
    (funcall predicate)))

(defun support-tests-temp-root ()
  "Return a fresh temp directory, registered for the story's cleanup."
  (let ((root (make-temp-file "pearl-root" t)))
    (push root ert-gwt--tracked-files)
    root))

(defun support-tests-git-root ()
  "Return a fresh temp directory initialized as a git repository.
The snapshot layer is a dangling commit, so a story about snapshots
needs a repository; `pearl-undo-snapshot' returns nil outside one."
  (let* ((root (support-tests-temp-root))
         (default-directory root))
    (call-process "git" nil nil nil "init" "-q")
    (call-process "git" nil nil nil "config" "user.email" "story@example.com")
    (call-process "git" nil nil nil "config" "user.name" "Story")
    root))

(defun support-tests-tcp-server (on-open)
  "Listen on a local port and call ON-OPEN with each accepted client.
Return the server process; the port is (process-contact SERVER :service).
The story closes it with `support-tests-tcp-close' in its cleanup."
  (make-network-process
   :name "pearl-story-server" :server t :host "127.0.0.1" :service 0
   :family 'ipv4 :noquery t
   :sentinel (lambda (client _event)
               (when (eq (process-status client) 'open)
                 (process-put client 'pearl-story-server t)
                 (funcall on-open client)))))

(defun support-tests-tcp-close (server)
  "Delete SERVER and every client connection it accepted."
  (dolist (proc (process-list))
    (when (and (process-live-p proc)
               (or (eq proc server)
                   (process-get proc 'pearl-story-server)))
      (delete-process proc))))

(defun support-tests-http-request-then (on-request)
  "Return a one-request HTTP server whose handler is ON-REQUEST.
ON-REQUEST receives the client and the request text, and sends the
response itself.  It is called once the blank line after the headers has
arrived, which is enough to assert on a method, a path or an
Authorization header without parsing anything."
  (support-tests-tcp-server
   (lambda (client)
     (process-put client 'pearl-story-request "")
     (set-process-filter
      client
      (lambda (process chunk)
        (let ((request (concat (process-get process 'pearl-story-request)
                               chunk)))
          (process-put process 'pearl-story-request request)
          (when (string-match-p "\r\n\r\n" request)
            (set-process-filter process nil)
            (funcall on-request process request))))))))

(defun support-tests-sse-frame (payload)
  "Return PAYLOAD as one complete SSE data frame."
  (concat "data: " payload "\n\n"))

(defun support-tests-sse-head (&optional status)
  "Return the head of a streaming response, ending at the blank line.
The body follows it, frame by frame.  STATUS defaults to 200 OK."
  (concat (format "HTTP/1.1 %s\r\n" (or status "200 OK"))
          "Content-Type: text/event-stream\r\n"
          "Connection: close\r\n"
          "\r\n"))

(defun support-tests-http-response (status body &optional headers)
  "Return a complete HTTP/1.1 response with STATUS and BODY.
HEADERS is an alist of extra lines, each (NAME . VALUE), for the
answers a story needs to be specific: Retry-After on a 429, say."
  (concat (format "HTTP/1.1 %s\r\n" status)
          "Content-Type: application/json\r\n"
          (format "Content-Length: %d\r\n" (string-bytes body))
          (mapconcat (lambda (header)
                       (format "%s: %s\r\n" (car header) (cdr header)))
                     headers "")
          "Connection: close\r\n"
          "\r\n"
          body))

(defun support-tests-openai-chunk (delta &optional finish)
  "Return one OpenAI-compatible chat-completions chunk carrying DELTA.
DELTA is the `delta' object as a plist, FINISH the `finish_reason'."
  (support-tests-sse-frame
   (json-serialize
    (list :id "chatcmpl-story" :object "chat.completion.chunk"
          :created 1 :model "story"
          :choices (vector (list :index 0
                                 :delta delta
                                 :finish_reason (or finish :null)))))))

(defun support-tests-openai-tool-call (index id name arguments)
  "Return one chunk fragment for the tool call at INDEX.
ID is the call id and NAME the tool, both sent once; ARGUMENTS is the
next piece of the object.  A nil field is omitted, which is how the
endpoint sends the fragments after the first one."
  (support-tests-openai-chunk
   (list :tool_calls
         (vector (append (list :index index)
                         (when id (list :id id :type "function"))
                         (list :function
                               (append (when name (list :name name))
                                       (list :arguments arguments))))))))

(defun support-tests-openai-done ()
  "Return the frame that terminates an OpenAI-compatible stream."
  (support-tests-sse-frame "[DONE]"))

(defmacro support-tests--stub (name)
  "Make NAME a function that reports it is not implemented.
Does nothing when NAME is already defined, so a landed module wins."
  `(unless (fboundp ',name)
     (defalias ',name
       (lambda (&rest _)
         (error "PEARL: %s is not implemented yet" ',name)))))

;; contract
(support-tests--stub pearl-contract-message)
(support-tests--stub pearl-contract-message-p)
(support-tests--stub pearl-contract-text)
(support-tests--stub pearl-contract-tool-call)
(support-tests--stub pearl-contract-parse-arguments)
(support-tests--stub pearl-contract-tool-result)
(support-tests--stub pearl-contract-tool-declaration)
(support-tests--stub pearl-contract-to-wire)
(support-tests--stub pearl-contract-from-wire)
(support-tests--stub pearl-contract-entry)

;; event
(support-tests--stub pearl-event-kinds)
(support-tests--stub pearl-event-emitter)
(support-tests--stub pearl-event-new)
(support-tests--stub pearl-event-subscribe)
(support-tests--stub pearl-event-emit)
(support-tests--stub pearl-event-render-hint)

;; loop
(support-tests--stub pearl-loop-new)
(support-tests--stub pearl-loop-submit)
(support-tests--stub pearl-loop-steer)
(support-tests--stub pearl-loop-follow-up)
(support-tests--stub pearl-loop-abort)
(support-tests--stub pearl-loop-busy-p)
(support-tests--stub pearl-loop-queued)
(support-tests--stub pearl-loop-subscribe)
(support-tests--stub pearl-loop-set-tools)

;; session
(support-tests--stub pearl-session-new)
(support-tests--stub pearl-session-append)
(support-tests--stub pearl-session-entries)
(support-tests--stub pearl-session-head)
(support-tests--stub pearl-session-parent)
(support-tests--stub pearl-session-set-head)
(support-tests--stub pearl-session-branch)
(support-tests--stub pearl-session-projection)
(support-tests--stub pearl-session-default-prompt)
(support-tests--stub pearl-session-patch)
(support-tests--stub pearl-session-rollback-plan)
(support-tests--stub pearl-session-conflict-p)

;; tool
(support-tests--stub pearl-tool-define)
(support-tests--stub pearl-tool-list)
(support-tests--stub pearl-tool-declarations)
(support-tests--stub pearl-tool-validate)
(support-tests--stub pearl-tool-run)
(support-tests--stub pearl-tool-error-result)
(support-tests--stub pearl-tool-truncate)

;; provider
(support-tests--stub pearl-provider-request)
(support-tests--stub pearl-provider-cancel)
(support-tests--stub pearl-provider-fake)
(support-tests--stub pearl-provider-fake-requests)
(support-tests--stub pearl-provider-p)

;; provider-sse
(support-tests--stub pearl-provider-sse)
(support-tests--stub pearl-provider-sse-headers)
(support-tests--stub pearl-provider-sse-request-body)
(support-tests--stub pearl-provider-sse-parse-chunk)
(support-tests--stub pearl-provider-sse-parse-event)
(support-tests--stub pearl-provider-sse-parse-head)
(support-tests--stub pearl-provider-sse-endpoint)
(support-tests--stub pearl-provider-sse-api-key)
(support-tests--stub pearl-provider-sse-tool-call-delta)
(support-tests--stub pearl-provider-sse-cancel)

;; proc
(support-tests--stub pearl-proc-run)
(support-tests--stub pearl-proc-check)
(support-tests--stub pearl-proc-kill)

;; session-jsonl
(support-tests--stub pearl-session-jsonl-open)
(support-tests--stub pearl-session-jsonl-append)
(support-tests--stub pearl-session-jsonl-read)
(support-tests--stub pearl-session-jsonl-close)
(support-tests--stub pearl-session-jsonl-path-for)

;; console
(support-tests--stub pearl-console-open)
(support-tests--stub pearl-console-wire)
(support-tests--stub pearl-console-mode)
(support-tests--stub pearl-console-send)
(support-tests--stub pearl-console-project-context)
(support-tests--stub pearl-console-model)
(support-tests--stub pearl-console-set-model)
(support-tests--stub pearl-console-api-key)
(support-tests--stub pearl-console-insert-event)
(support-tests--stub pearl-console-status)
(support-tests--stub pearl-console-abort)
(support-tests--stub pearl-console-redraw)
(support-tests--stub pearl-console-rollback)
(support-tests--stub pearl-console-close)

;; review
(support-tests--stub pearl-review-install)
(support-tests--stub pearl-review-parse)
(support-tests--stub pearl-review-fold-p)
(support-tests--stub pearl-review-locate)
(support-tests--stub pearl-review-xref-backend)

;; tools
(support-tests--stub pearl-tools-read-declaration)
(support-tests--stub pearl-tools-read-run)
(support-tests--stub pearl-tools-read-install)
(support-tests--stub pearl-tools-edit-declaration)
(support-tests--stub pearl-tools-edit-run)
(support-tests--stub pearl-tools-edit-apply)
(support-tests--stub pearl-tools-edit-unsaved-p)
(support-tests--stub pearl-tools-edit-install)
(support-tests--stub pearl-tools-shell-declaration)
(support-tests--stub pearl-tools-shell-run)
(support-tests--stub pearl-tools-shell-confirm)
(support-tests--stub pearl-tools-shell-scope)
(support-tests--stub pearl-tools-shell-install)

;; undo
(support-tests--stub pearl-undo-group-prepare)
(support-tests--stub pearl-undo-group-accept)
(support-tests--stub pearl-undo-group-cancel)
(support-tests--stub pearl-undo-snapshot)
(support-tests--stub pearl-undo-restore)
(support-tests--stub pearl-undo-conflict-p)
(support-tests--stub pearl-undo-attach)
(support-tests--stub pearl-undo-turn-begin)
(support-tests--stub pearl-undo-turn-end)
(support-tests--stub pearl-undo-turn-snapshot)
(support-tests--stub pearl-undo-latest)

(provide 'support-tests)

;;; support-tests.el ends here
