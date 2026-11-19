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

(defun support-tests-perf-scale ()
  "Return the factor the latency stories apply to their budgets.
1.0 unless PEARL_PERF_SCALE says otherwise.  The number written in a story
is the acceptance; the factor exists so a loaded machine can say so in the
environment instead of someone lowering the constant in a commit where
nobody would see it.

A tag would be the better mechanism and is not available: the vendored
`ert-gwt-deftest' takes clause keywords only, and the runner in `ci/'
selects nothing, so a tag here would be inert."
  (let ((value (getenv "PEARL_PERF_SCALE")))
    (if (and value (string-match-p "\\`[0-9]+\\(?:\\.[0-9]+\\)?\\'" value))
        (string-to-number value)
      1.0)))

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

;; event

;; turn

;; loop

;; session

;; prompt

;; compaction

;; tool

;; provider

;; provider-sse

;; proc

;; session-jsonl

;; app

;; console

;; review

;; tools

;; delegate

;; undo

(provide 'support-tests)

;;; support-tests.el ends here
