;;; pearl-provider-sse.el --- one streaming HTTP client  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The only module that speaks HTTP, and the only one that knows a vendor's
;; streaming protocol.  It implements `pearl-provider' and stops there: what a
;; delta means is the loop's business, what to do with a timeout is the
;; provider's.
;;
;; The protocol is the OpenAI-compatible chat-completions stream, per ADR 7:
;; a POST to BASE-URL/chat/completions with `stream' true, answered by
;; server-sent events whose data lines are `chat.completion.chunk' objects.
;; Text and tool calls live in `choices[0].delta', the end of a turn in
;; `finish_reason', and `data: [DONE]' terminates the body.  A vendor's extra
;; fields are ignored, which is what keeps one adapter working against the
;; several endpoints that copied this shape.
;;
;; Three limits are part of the interface rather than a detail: an idle limit
;; (a connection that stops producing), a total limit (a stream that never
;; stops producing), and a byte limit (a stream that produces too much).  A
;; request that exceeds one of them is abandoned by name, so a user sees which
;; limit fired.  The body is counted, not the head: the limits are about what
;; the model is saying.  Each attempt gets its own limits, because a retry
;; that inherited a spent clock would fail for the attempt before it.
;;
;; The scheme in BASE-URL decides the transport: https is opened as TLS, so
;; Emacs's own trust store decides whether the endpoint is who it says, and a
;; handshake that fails is `(:reason tls-error)'.  There is no insecure
;; fallback and no flag to add one; a key that goes out in clear text is worse
;; than a request that does not go out.
;;
;; Two rules make a failure say what happened (ADR 18).  A 429 or a 503 is
;; retried, because both are transient and neither is the request's fault,
;; while any other status ends the request.  And a retry stops at the first
;; delivered event: once the model has said something, a broken connection is
;; `(:reason truncated)' and what arrived stays, because asking again would
;; render the same paragraph twice and re-run tool calls that already landed.
;;
;; The key is resolved where the user already keeps credentials and is never
;; stored by PEARL (ADR 17), and it reaches the wire in one function.
;;
;; Planned API
;;
;; `pearl-provider-sse-client' (&key BASE-URL API-KEY MODEL IDLE-TIMEOUT
;;      TOTAL-TIMEOUT BYTE-LIMIT RETRY-LIMIT) -> provider
;;      Builds a provider that satisfies `pearl-provider-p'.  BASE-URL is the
;;      prefix the endpoint ends, such as "https://api.openai.com/v1"; the
;;      request path is appended to it.  Defaults live in one `defcustom'
;;      block, because these are the numbers a user changes.  API-KEY nil
;;      means "resolve it", not "send none".  MODEL is the fallback for a
;;      request that carries no model: the console's choice travels in the
;;      request, so changing it mid-session changes the next request.
;;      RETRY-LIMIT counts additional attempts after the first, for 429 and
;;      503 only, and zero means ask once.
;;
;; `pearl-provider-sse-endpoint' (BASE-URL) -> plist
;;      The connection the URL names: `(:scheme https :host "api.example.com"
;;      :port 443 :path "/v1")'.  An explicit port wins over the scheme's
;;      default, and the path keeps its trailing form so the request path can
;;      be appended without guessing about slashes.  A URL with no scheme
;;      signals `pearl-provider-invalid-url': guessing between http and https
;;      is how a key goes out in clear text.
;;
;; TODO `pearl-provider-sse-base-url' -> defcustom, the endpoint prefix
;;      Every request goes here.  The default is the public
;;      OpenAI-compatible endpoint, and a user pointing PEARL at a compatible
;;      one sets this and nothing else.  `pearl-provider-sse-client' takes its
;;      BASE-URL from this when the argument is nil, so a console built with no
;;      provider of its own needs no Lisp to reach a different vendor.
;;
;; `pearl-provider-sse-api-key' (HOST) -> string
;;      The key for HOST, in the order ADR 17 fixes: `auth-source' for the
;;      host, then `PEARL_API_KEY', then one `read-passwd' prompt memoized for
;;      the session.  Signals `pearl-api-key-missing' naming HOST and the two
;;      lookups rather than sending an empty bearer.
;;
;; `pearl-provider-sse-headers' (API-KEY) -> alist
;;      The request headers: `Authorization' as "Bearer KEY",
;;      `Content-Type: application/json', and the accept header that asks for
;;      a stream.  Authorization is built here and logged nowhere.  The
;;      connection itself is opened by `pearl-provider-request' with
;;      `open-network-stream', its `:type' taken from the endpoint's scheme.
;;
;; `pearl-provider-sse-payload' (MESSAGE) -> plist
;;      One message as the chat-completions body wants it: `:role' spelled the
;;      way the endpoint spells roles, and `:content' or `:tool_calls'
;;      depending on what the message carries.  The conversion lives in the
;;      adapter because the field names are the vendor's; a core that speaks
;;      them has to change when a vendor renames one (ADR 7).
;;
;; `pearl-provider-sse-read-payload' (PAYLOAD) -> message
;;      The inverse, and the reason a story can assert on what the model would
;;      have been told: a test builds the body an endpoint receives and reads
;;      the messages back out of it.
;;
;; `pearl-provider-sse-request-body' (REQUEST MODEL) -> plist
;;      The chat-completions body for REQUEST: `:model', `:messages' as the
;;      payloads `pearl-provider-sse-payload' writes, `:stream' true, and
;;      `:tools' from the declarations when the request carries any.  A
;;      vendor's reasoning field is not built here; it arrives as an ignored
;;      extra if the endpoint sends one.
;;
;; `pearl-provider-sse-parse-head' (TEXT) -> (STATUS . REST) or nil
;;      Consume the status line and headers of the HTTP response and return
;;      the status code and whatever body arrived in the same chunk.  Nil
;;      means the head is still incomplete.  A status outside 2xx is reported
;;      as `(:reason http-error :status STATUS :body ERROR)' once the body has
;;      arrived, because that is what a wrong key looks like, and the caller
;;      decides from STATUS whether another attempt is worth spending (ADR 18).
;;      ERROR is the body decoded when it is JSON, so a caller reads the
;;      endpoint's own `message', and the raw text when it is not.  A 400 whose
;;      body says the request exceeded the context window is
;;      `(:reason overflow :status 400)' instead, because that is the one
;;      status a caller can act on by compacting rather than by retrying or
;;      reporting (ADR 27).
;;
;; `pearl-provider-sse-parse-chunk' (PARTIAL CHUNK) -> (EVENTS . REST)
;;      Split a network chunk into complete SSE events and the incomplete tail.
;;      EVENTS is the list of what `pearl-provider-sse-parse-event' returned
;;      for each completed frame, in arrival order, and REST is the raw tail to
;;      prepend to the next chunk.  A blank line ends an event, and one event
;;      may carry several `data:' lines.  A chunk boundary can fall anywhere,
;;      including inside an event and inside a UTF-8 sequence; that is the
;;      reason this is a function with a test rather than a line in a filter.
;;
;; `pearl-provider-sse-parse-event' (STRING) -> event plist or nil
;;      One OpenAI-compatible chunk into the loop's vocabulary:
;;      `(:type text :text …)' from `choices[0].delta.content',
;;      `(:type tool-call :index N :id … :name … :arguments …)' from
;;      `choices[0].delta.tool_calls', and `(:type stop :reason SYMBOL)' from
;;      `finish_reason', with `length' preserved because a truncated turn must
;;      not run its tool calls.  `data: [DONE]' is `(:type done)', and it is
;;      the only clean end of a body: a close before it is
;;      `(:reason truncated)', because a proxy reset mid-token renders as a
;;      short answer if the client trusts the end of file.  An empty or
;;      unknown chunk, an unknown field and a malformed object are nil:
;;      vendors add events, and a parser that signals loses a working turn.
;;
;; `pearl-provider-sse-tool-call-delta' (STATE DELTA) -> STATE
;;      Concatenate a partial tool call across deltas, keyed by the `index'
;;      the endpoint repeats in every fragment.  STATE is the list of calls in
;;      index order, each `(:index N :id … :name … :arguments …)', and a new
;;      index appends to it.  The id and the name arrive once and the
;;      `arguments' string arrives in pieces, so matching on id loses the
;;      second fragment and a parser that assumes one event produces a
;;      truncated JSON object and a model that appears to lie.
;;
;; `pearl-provider-sse-cancel' (HANDLE) -> nil
;;      Delete the process and mark the request abandoned, so a late event is
;;      dropped instead of rendered.

;;; Code:

(require 'auth-source)
(require 'cl-lib)
(require 'json)
(require 'seq)
(require 'pearl-contract)
(require 'pearl-provider)

(defgroup pearl-provider-sse nil
  "The streaming HTTP client the package talks to a model with."
  :group 'pearl)

(defcustom pearl-provider-sse-base-url "https://api.openai.com/v1"
  "The prefix every request is sent to."
  :type 'string
  :group 'pearl-provider-sse)

(defcustom pearl-provider-sse-idle-timeout 120
  "Seconds a connection may produce nothing before it is abandoned."
  :type 'number
  :group 'pearl-provider-sse)

(defcustom pearl-provider-sse-total-timeout 600
  "Seconds one attempt may take, however busy it is."
  :type 'number
  :group 'pearl-provider-sse)

(defcustom pearl-provider-sse-byte-limit 8388608
  "Bytes one attempt may receive."
  :type 'integer
  :group 'pearl-provider-sse)

(defcustom pearl-provider-sse-retry-limit 2
  "Attempts beyond the first, for 429 and 503 only.

Zero asks once, which is what a request the user can see should default
to when the endpoint is theirs."
  :type 'integer
  :group 'pearl-provider-sse)

(defvar pearl-provider-sse-key-cache nil
  "Keys typed during this session, as an alist of host and secret.")

(defvar pearl-provider-sse-read-passwd #'read-passwd
  "How a key is asked for when no source has one.

A variable rather than a call, so a story can answer without a terminal
and a user can point it at their own prompt.")

(cl-defstruct (pearl-provider-sse--session (:constructor pearl-provider-sse--session-create))
  "The one request this provider is carrying out."
  (process nil)
  (run nil)
  (finished nil)
  (on-done #'ignore)
  (on-error #'ignore))

(defun pearl-provider-sse-endpoint (base-url)
  "The connection BASE-URL names, as a plist.

`:scheme', `:host', `:port' and `:path', with the port an explicit one
wins over the scheme's default.  A URL with no scheme is refused rather
than guessed at: picking http for a https endpoint is how a key goes out
in clear text."
  (unless (string-match "\\`\\([a-zA-Z]+\\)://\\([^/:?#]+\\)\\(?::\\([0-9]+\\)\\)?\\(/[^?#]*\\)?" base-url)
    (signal 'pearl-provider-invalid-url (list base-url)))
  (let* ((scheme (downcase (match-string 1 base-url)))
         (host (match-string 2 base-url))
         (port (if (match-string 3 base-url)
                   (string-to-number (match-string 3 base-url))
                 (pcase scheme
                   ("https" 443)
                   ("http" 80)
                   (_ (signal 'pearl-provider-invalid-url (list base-url))))))
         (path (or (match-string 4 base-url) "/")))
    (list :scheme (intern scheme) :host host :port port :path path)))

(defun pearl-provider-sse-headers (api-key)
  "The headers one request carries, with API-KEY as a bearer token.

The key is built into the Authorization line here and appears in no log,
no message and no buffer."
  (list (cons "Authorization" (format "Bearer %s" api-key))
        (cons "Content-Type" "application/json")
        (cons "Accept" "text/event-stream")))

(defun pearl-provider-sse--arguments-string (arguments)
  "ARGUMENTS as the endpoint wants them: a JSON object in a string."
  (cond ((null arguments) "{}")
        ((stringp arguments) arguments)
        (t (json-serialize arguments))))

(defun pearl-provider-sse-payload (message)
  "MESSAGE as one entry of the endpoint's `messages' array.

A message becomes `:role' and `:content', a tool call becomes an
assistant turn carrying `:tool_calls', because the field names are the
vendor's and the core does not speak them (ADR 7)."
  (if (plist-get message :name)
      (list :role "assistant"
            :tool_calls (vector (list :id (plist-get message :id)
                                      :type "function"
                                      :function (list :name (plist-get message :name)
                                                      :arguments
                                                      (pearl-provider-sse--arguments-string
                                                       (plist-get message :arguments))))))
    (list :role (format "%s" (plist-get message :role))
          :content (pearl-contract-text message))))

(defun pearl-provider-sse-read-payload (payload)
  "The message PAYLOAD carries, back in the core's vocabulary."
  (if-let* ((calls (plist-get payload :tool_calls)))
      (let* ((call (aref calls 0))
             (function (plist-get call :function))
             (arguments (plist-get function :arguments)))
        (pearl-contract-tool-call
         (plist-get call :id)
         (plist-get function :name)
         (if (stringp arguments)
             (condition-case nil
                 (json-parse-string arguments :object-type 'alist :array-type 'list
                                    :null-object nil :false-object nil)
               (json-error nil))
           arguments)))
    (pearl-contract-message (intern (plist-get payload :role))
                            (or (plist-get payload :content) ""))))

(defun pearl-provider-sse-request-body (request model)
  "The chat-completions body for REQUEST, whose MODEL is the fallback.

`:tools' is left out when the request declares none: an empty array is a
declaration the model is offered and cannot call."
  (let ((body (list :model (or (plist-get request :model) model)
                    ;; A vector, not a list: `json-serialize' reads a
                    ;; list whose first element is a list as an alist,
                    ;; and announces the mistake with the first value it
                    ;; finds there.
                    :messages (vconcat
                               (mapcar #'pearl-provider-sse-payload
                                       (plist-get request :messages)))
                    :stream t)))
    (if-let* ((tools (plist-get request :tools)))
        (append body (list :tools (vconcat tools)))
      body)))

(defun pearl-provider-sse-parse-head (text)
  "Consume the status line and headers of TEXT.

Return `(STATUS . REST)' where REST is whatever arrived after the blank
line, or nil when the head is still incomplete."
  (when (string-match "\r\n\r\n\\|\n\n" text)
    (let ((head (substring text 0 (match-beginning 0)))
          (rest (substring text (match-end 0))))
      (when (string-match "\\`HTTP/[0-9.]+ +\\([0-9]+\\)" head)
        (cons (string-to-number (match-string 1 head)) rest)))))

(defun pearl-provider-sse-parse-chunk (partial chunk)
  "Split the concatenation of PARTIAL and CHUNK into `(EVENTS . REST)'.

A blank line ends a frame, so what is left over is the tail of one the
next chunk will finish."
  (let ((text (concat partial chunk))
        (events nil)
        (start 0))
    (while (string-match "\n\n" text start)
      (let ((frame (substring text start (match-beginning 0))))
        (setq start (match-end 0))
        (dolist (event (pearl-provider-sse--frame-events (string-trim frame)))
          (push event events))))
    (cons (nreverse events) (substring text start))))

(defun pearl-provider-sse--frame-events (string)
  "Every event STRING carries, in the order the loop should see them.

Usually one.  A frame that ends the turn and carries text at the same
time is two, because dropping either loses something the caller needs."
  (when (string-prefix-p "data:" string)
    (let ((data (string-trim (substring string (length "data:")))))
      (cond
       ((equal data "[DONE]") '((:type done)))
       ((equal data "") nil)
       (t
        (let ((object (condition-case nil
                          (json-parse-string data :object-type 'plist
                                             :array-type 'list
                                             :null-object nil :false-object nil)
                        (json-error nil))))
          (when object
            (let* ((choice (car (plist-get object :choices)))
                   (delta (plist-get choice :delta))
                   (content (plist-get delta :content))
                   (calls (plist-get delta :tool_calls))
                   (finish (plist-get choice :finish_reason))
                   (events nil))
              (when (and (stringp content) (not (equal content "")))
                (push (list :type 'text :text content) events))
              (when calls
                (let* ((fragment (car calls))
                       (function (plist-get fragment :function)))
                  (push (list :type 'tool-call
                              :index (plist-get fragment :index)
                              :id (plist-get fragment :id)
                              :name (plist-get function :name)
                              :arguments (plist-get function :arguments))
                        events)))
              (when finish
                (push (list :type 'stop
                            :reason (pcase finish
                                      ("stop" 'end)
                                      ("length" 'length)
                                      ("tool_calls" 'tool-calls)
                                      (_ 'error)))
                      events))
              (nreverse events)))))))))

(defun pearl-provider-sse-parse-event (string)
  "One OpenAI-compatible chunk, STRING, into the loop's vocabulary.

A frame that says nothing, or something this package does not know, is
nil: a vendor adds events, and a parser that signals loses a turn that
was working."
  (car (pearl-provider-sse--frame-events string)))

(defun pearl-provider-sse--merge-call (call delta)
  "CALL grown by DELTA, keeping the id and name the first fragment carried."
  (list :index (plist-get delta :index)
        :id (or (plist-get delta :id) (plist-get call :id))
        :name (or (plist-get delta :name) (plist-get call :name))
        :arguments (concat (or (plist-get call :arguments) "")
                           (or (plist-get delta :arguments) ""))))

(defun pearl-provider-sse-tool-call-delta (state delta)
  "Concatenate DELTA into STATE, matching by the index the endpoint repeats.

The id and the name arrive once and the arguments arrive in pieces, so
matching on the id loses every fragment after the first."
  (if (not (eq (plist-get delta :type) 'tool-call))
      state
    (let ((tail state)
          (index (plist-get delta :index))
          (found nil))
      (while (and tail (not found))
        (if (equal (plist-get (car tail) :index) index)
            (setq found t)
          (setq tail (cdr tail))))
      (if found
          (setcar tail (pearl-provider-sse--merge-call (car tail) delta))
        (setq state (append state (list (pearl-provider-sse--merge-call nil delta)))))
      state)))

(defun pearl-provider-sse-api-key (host)
  "The key for HOST, resolved in the order ADR 17 fixes.

`auth-source' first, then `PEARL_API_KEY', then one prompt memoized for
the session.  Nothing is written anywhere, and a host with no key
signals `pearl-api-key-missing' rather than sending an empty bearer."
  (or (cdr (assoc host pearl-provider-sse-key-cache))
      (let ((found (car (auth-source-search :host host :max 1 :require '(:secret))))
            (key nil))
        (when found
          (let ((secret (plist-get found :secret)))
            (setq key (if (functionp secret) (funcall secret) secret))))
        (when (and (null key) (getenv "PEARL_API_KEY"))
          (setq key (getenv "PEARL_API_KEY")))
        (when (null key)
          (setq key (funcall pearl-provider-sse-read-passwd
                             (format "API key for %s: " host))))
        (when (null key)
          (signal 'pearl-api-key-missing (list host)))
        (push (cons host key) pearl-provider-sse-key-cache)
        key)))

(defun pearl-provider-sse-client (&rest keys)
  "Build a provider that streams from an OpenAI-compatible endpoint.

KEYS carries `:base-url', `:api-key', `:model', `:idle-timeout',
`:total-timeout', `:byte-limit' and `:retry-limit'; each defaults to the
`defcustom' above.  A nil `:base-url' means the customised one, which is
what makes a different vendor a setting and not a Lisp form."
  (list :pearl-provider t
        :kind 'sse
        :base-url (or (plist-get keys :base-url) pearl-provider-sse-base-url)
        :api-key (plist-get keys :api-key)
        :model (plist-get keys :model)
        :idle-timeout (or (plist-get keys :idle-timeout) pearl-provider-sse-idle-timeout)
        :total-timeout (or (plist-get keys :total-timeout) pearl-provider-sse-total-timeout)
        :byte-limit (or (plist-get keys :byte-limit) pearl-provider-sse-byte-limit)
        :retry-limit (or (plist-get keys :retry-limit) pearl-provider-sse-retry-limit)
        :session (pearl-provider-sse--session-create)
        :request #'pearl-provider-sse--request
        :cancel #'pearl-provider-sse--cancel))

(defun pearl-provider-sse--retry-after (head)
  "The seconds HEAD asks the client to wait, or zero."
  (if (and (stringp head)
           (string-match "Retry-After: *\\([0-9]+\\)" head))
      (string-to-number (match-string 1 head))
    0))

(defun pearl-provider-sse--decode (text)
  "TEXT as the endpoint's error object when it is JSON, else as TEXT."
  (let ((parsed (condition-case nil
                    (json-parse-string text :object-type 'plist
                                       :array-type 'list :null-object nil :false-object nil)
                  (error nil))))
    (if (and parsed (consp parsed)) parsed text)))

(defun pearl-provider-sse--overflow-p (text)
  "Non-nil when the error TEXT reports the window was exceeded."
  (and (stringp text)
       (string-match-p "context\\(_length\\| window\\| length\\)\\|maximum context\\|too many tokens"
                       (downcase text))
       t))

(cl-defstruct (pearl-provider-sse--run (:constructor pearl-provider-sse--run-create))
  "One attempt at one request, and everything it has read so far."
  provider session request emit attempts process
  (pending "") (status nil) (head "") (error-body "") (length nil)
  (delivered nil) (done-seen nil) (finished nil) (bytes 0) (idle nil) (total nil))

(defun pearl-provider-sse--stop (provider)
  "Abandon PROVIDER's request, calling the done callback once."
  (let ((session (plist-get provider :session)))
    (unless (pearl-provider-sse--session-finished session)
      (setf (pearl-provider-sse--session-finished session) t)
      (let ((run (pearl-provider-sse--session-run session))
            (process (pearl-provider-sse--session-process session)))
        (when run (setf (pearl-provider-sse--run-finished run) t))
        (when (process-live-p process) (delete-process process)))
      (funcall (pearl-provider-sse--session-on-done session)))))

(defun pearl-provider-sse-cancel (handle)
  "Stop the request HANDLE names, without reporting to the caller.

`pearl-provider-cancel' is the interface and releases the caller; this
is the adapter's own stop, for a console that is already gone."
  (when (pearl-provider-sse--run-p handle)
    (let ((run handle))
      (setf (pearl-provider-sse--run-finished run) t)
      (setf (pearl-provider-sse--session-finished (pearl-provider-sse--run-session run)) t)
      (let ((process (pearl-provider-sse--run-process run)))
        (when (process-live-p process) (delete-process process)))))
  nil)

(defun pearl-provider-sse--cancel (provider _handle)
  "Abandon PROVIDER's request, releasing the caller once."
  (pearl-provider-sse--stop provider))

(defun pearl-provider-sse--run-shut (run)
  "Stop RUN: no more chunks and no more timers."
  (setf (pearl-provider-sse--run-finished run) t)
  (let ((idle (pearl-provider-sse--run-idle run))
        (total (pearl-provider-sse--run-total run))
        (process (pearl-provider-sse--run-process run)))
    (when (timerp idle) (cancel-timer idle))
    (when (timerp total) (cancel-timer total))
    (when (process-live-p process) (delete-process process))))

(defun pearl-provider-sse--run-succeed (run)
  "End RUN as the protocol ends it."
  (unless (pearl-provider-sse--run-finished run)
    (pearl-provider-sse--run-shut run)
    (setf (pearl-provider-sse--session-finished (pearl-provider-sse--run-session run)) t)
    (funcall (pearl-provider-sse--session-on-done
              (pearl-provider-sse--run-session run)))))

(defun pearl-provider-sse--run-fail (run condition)
  "End RUN with CONDITION, telling the caller once."
  (unless (pearl-provider-sse--run-finished run)
    (pearl-provider-sse--run-shut run)
    (let ((session (pearl-provider-sse--run-session run)))
      (setf (pearl-provider-sse--session-finished session) t)
      (funcall (pearl-provider-sse--session-on-error session) condition)
      (funcall (pearl-provider-sse--session-on-done session)))))

(defun pearl-provider-sse--run-touch (run)
  "Restart RUN's idle clock, because something arrived."
  (let ((idle (pearl-provider-sse--run-idle run)))
    (when (timerp idle) (cancel-timer idle)))
  (setf (pearl-provider-sse--run-idle run)
        (run-at-time (plist-get (pearl-provider-sse--run-provider run) :idle-timeout)
                     nil
                     (lambda () (pearl-provider-sse--run-fail run (list :reason 'idle-timeout))))))

(defun pearl-provider-sse--run-feed (run text)
  "Deliver the events RUN has in TEXT, and notice the terminator."
  (let* ((parsed (pearl-provider-sse-parse-chunk (pearl-provider-sse--run-pending run) text)))
    (setf (pearl-provider-sse--run-pending run) (cdr parsed))
    (dolist (event (car parsed))
      (if (eq (plist-get event :type) 'done)
          (setf (pearl-provider-sse--run-done-seen run) t)
        (setf (pearl-provider-sse--run-delivered run) t)
        (funcall (pearl-provider-sse--run-emit run) event))))
  (when (pearl-provider-sse--run-done-seen run)
    (pearl-provider-sse--run-succeed run)))

(defun pearl-provider-sse--run-http-failed (run)
  "Decide what a status outside 2xx means for RUN."
  (let ((status (pearl-provider-sse--run-status run))
        (decoded (pearl-provider-sse--decode (pearl-provider-sse--run-error-body run)))
        (provider (pearl-provider-sse--run-provider run)))
    (cond
     ((and (equal status 400)
           (pearl-provider-sse--overflow-p (pearl-provider-sse--run-error-body run)))
      (pearl-provider-sse--run-fail run (list :reason 'overflow :status status :body decoded)))
     ((and (memq status '(429 503))
           (< (pearl-provider-sse--run-attempts run)
              (1+ (or (plist-get provider :retry-limit) 0))))
      (pearl-provider-sse--run-shut run)
      (run-at-time (pearl-provider-sse--retry-after (pearl-provider-sse--run-head run))
                   nil
                   (lambda () (pearl-provider-sse--run-start run))))
     (t
      (pearl-provider-sse--run-fail
       run (list :reason 'http-error :status status
                 :body (or (plist-get decoded :error) decoded)))))))

(defun pearl-provider-sse--run-body-complete-p (run)
  "Non-nil when RUN's error body is as long as its head promised."
  (let ((length (pearl-provider-sse--run-length run)))
    (or (null length)
        (>= (string-bytes (pearl-provider-sse--run-error-body run)) length))))

(defun pearl-provider-sse--run-receive (run chunk)
  "Take CHUNK as it arrives for RUN: head first, then the body."
  (unless (pearl-provider-sse--run-finished run)
    (pearl-provider-sse--run-touch run)
    (setf (pearl-provider-sse--run-bytes run)
          (+ (pearl-provider-sse--run-bytes run) (string-bytes chunk)))
    (let ((limit (plist-get (pearl-provider-sse--run-provider run) :byte-limit)))
      (when (and limit (> (pearl-provider-sse--run-bytes run) limit))
        (pearl-provider-sse--run-fail run (list :reason 'byte-limit
                                                :bytes (pearl-provider-sse--run-bytes run)))))
    (unless (pearl-provider-sse--run-finished run)
      (if (null (pearl-provider-sse--run-status run))
          (let ((head (concat (pearl-provider-sse--run-head run) chunk)))
            (setf (pearl-provider-sse--run-head run) head)
            (let ((parsed (pearl-provider-sse-parse-head head)))
              (when parsed
                (setf (pearl-provider-sse--run-status run) (car parsed))
                (when (string-match "Content-Length: *\\([0-9]+\\)" head)
                  (setf (pearl-provider-sse--run-length run)
                        (string-to-number (match-string 1 head))))
                (let ((status (pearl-provider-sse--run-status run)))
                  (if (and (>= status 200) (< status 300))
                      (pearl-provider-sse--run-feed run (cdr parsed))
                    (setf (pearl-provider-sse--run-error-body run) (cdr parsed))
                    (when (pearl-provider-sse--run-body-complete-p run)
                      (pearl-provider-sse--run-http-failed run)))))))
        (let ((status (pearl-provider-sse--run-status run)))
          (if (and (>= status 200) (< status 300))
              (pearl-provider-sse--run-feed run chunk)
            (setf (pearl-provider-sse--run-error-body run)
                  (concat (pearl-provider-sse--run-error-body run) chunk))
            (when (pearl-provider-sse--run-body-complete-p run)
              (pearl-provider-sse--run-http-failed run))))))))

(defun pearl-provider-sse--run-sentinel (run process _event)
  "End RUN on a close of PROCESS, only when the body finished first."
  (when (and (not (pearl-provider-sse--run-finished run))
             (memq (process-status process) '(closed failed)))
    ;; The last chunk can still be sitting in the pipe when the sentinel
    ;; runs, so read what is there before deciding how the body ended.
    (accept-process-output process 0)
    (let ((status (pearl-provider-sse--run-status run)))
      (cond
       ((and status (>= status 200) (< status 300))
        (if (pearl-provider-sse--run-done-seen run)
            (pearl-provider-sse--run-succeed run)
          (pearl-provider-sse--run-fail run (list :reason 'truncated))))
       ((and status (>= status 400)) (pearl-provider-sse--run-http-failed run))
       (t (pearl-provider-sse--run-fail run (list :reason 'connection-closed)))))))

(defun pearl-provider-sse--run-start (run)
  "Open the connection RUN needs and send its request."
  (setf (pearl-provider-sse--run-attempts run)
        (1+ (pearl-provider-sse--run-attempts run))
        (pearl-provider-sse--run-pending run) ""
        (pearl-provider-sse--run-head run) ""
        (pearl-provider-sse--run-error-body run) ""
        (pearl-provider-sse--run-status run) nil
        (pearl-provider-sse--run-length run) nil
        (pearl-provider-sse--run-delivered run) nil
        (pearl-provider-sse--run-done-seen run) nil
        (pearl-provider-sse--run-bytes run) 0
        (pearl-provider-sse--run-finished run) nil)
  (let* ((provider (pearl-provider-sse--run-provider run))
         (endpoint (pearl-provider-sse-endpoint
                    (or (plist-get provider :base-url) pearl-provider-sse-base-url)))
         (host (plist-get endpoint :host))
         (key (or (plist-get provider :api-key) (pearl-provider-sse-api-key host)))
         (path (concat (plist-get endpoint :path) "/chat/completions"))
         (body (json-serialize
                (pearl-provider-sse-request-body
                 (pearl-provider-sse--run-request run) (plist-get provider :model))))
         (process (make-network-process
                   :name "pearl-sse"
                   :buffer nil
                   :host host
                   :service (plist-get endpoint :port)
                   :type (when (eq (plist-get endpoint :scheme) 'https) 'tls)
                   :coding 'utf-8-unix
                   :nowait nil
                   :filter (lambda (_process chunk)
                             (pearl-provider-sse--run-receive run chunk))
                   :sentinel (lambda (process event)
                               (pearl-provider-sse--run-sentinel run process event)))))
    (setf (pearl-provider-sse--run-process run) process)
    (setf (pearl-provider-sse--session-process
           (pearl-provider-sse--run-session run)) process
          (pearl-provider-sse--session-run
           (pearl-provider-sse--run-session run)) run)
    (setf (pearl-provider-sse--run-total run)
          (run-at-time (plist-get provider :total-timeout) nil
                       (lambda () (pearl-provider-sse--run-fail run (list :reason 'total-timeout)))))
    (pearl-provider-sse--run-touch run)
    (when (memq (process-status process) '(open run listen))
      (process-send-string
       process
       (concat (format "POST %s HTTP/1.1\r\n" path)
               (format "Host: %s\r\n" host)
               (mapconcat (lambda (header)
                            (format "%s: %s\r\n" (car header) (cdr header)))
                          (pearl-provider-sse-headers key) "")
               (format "Content-Length: %d\r\n" (string-bytes body))
               "Connection: close\r\n\r\n"
               body)))))

(defun pearl-provider-sse--request (provider request emit keys)
  "POST REQUEST to PROVIDER and deliver its events to EMIT.

KEYS carries `:on-done' and `:on-error'.  Each attempt gets its own idle,
total and byte limits, because a retry that inherited a spent clock would
fail for the attempt before it."
  (let ((session (plist-get provider :session)))
    (setf (pearl-provider-sse--session-finished session) nil
          (pearl-provider-sse--session-on-done session) (or (plist-get keys :on-done) #'ignore)
          (pearl-provider-sse--session-on-error session) (or (plist-get keys :on-error) #'ignore)
          (pearl-provider-sse--session-process session) nil
          (pearl-provider-sse--session-run session) nil)
    (let ((run (pearl-provider-sse--run-create
                :provider provider :session session :request request :emit emit :attempts 0)))
      (pearl-provider-sse--run-start run)
      run)))

(provide 'pearl-provider-sse)

;;; pearl-provider-sse.el ends here
