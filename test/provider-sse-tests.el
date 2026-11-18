;;; provider-sse-tests.el --- stories about one stream  -*- lexical-binding: t; -*-

;;; Commentary:

;; The stream is the OpenAI-compatible chat-completions one (ADR 7), and the
;; key is resolved rather than stored (ADR 17).  A chunk boundary can fall
;; inside an event, and a wrong key looks like a stream that says nothing.

;;; Code:

(require 'support-tests)
(require 'pearl-provider-sse)
(require 'pearl-provider)
(require 'pearl-contract)
(require 'cl-lib)
(require 'ert-gwt)

;; A chunk carries complete events and an incomplete tail; the tail is
;; returned, not dropped, because the next chunk finishes it.
(ert-gwt-deftest
  (:given ((partial "")
           (whole (support-tests-openai-chunk '(:content "one")))
           (chunk (concat (support-tests-openai-chunk '(:role "assistant" :content ""))
                          (substring whole 0 20)))
           (parsed nil)))
  (:when (setq parsed (pearl-provider-sse-parse-chunk partial chunk)))
  (:then (null (car parsed)))
  (:then (equal (substring whole 0 20) (cdr parsed))))

;; A chunk carries one event whole: the parser returns it and no tail, and the
;; event is the loop's vocabulary, not the endpoint's.
(ert-gwt-deftest
  (:given ((parsed nil)))
  (:when (setq parsed (pearl-provider-sse-parse-chunk
                       "" (support-tests-openai-chunk '(:content "one")))))
  (:then (equal "one" (plist-get (car (car parsed)) :text)))
  (:then (equal "" (cdr parsed))))

;; A boundary inside an event: the first half parses to nothing, and the
;; second half completes the event the first half started.
(ert-gwt-deftest
  (:given ((frame (support-tests-openai-chunk '(:content "one")))
           (first-half (substring frame 0 24))
           (second-half (substring frame 24))
           (parsed nil) (finished nil)))
  (:when (progn (setq parsed (pearl-provider-sse-parse-chunk "" first-half))
                (setq finished (pearl-provider-sse-parse-chunk (cdr parsed) second-half))))
  (:then (null (car parsed)))
  (:then (equal "one" (plist-get (car (car finished)) :text)))
  (:then (equal "" (cdr finished))))

;; The head is the status line and the headers; the body that arrived with it
;; is handed on, and nothing is guessed from a head that is still arriving.
(ert-gwt-deftest
  (:given ((text (concat (support-tests-sse-head "200 OK")
                         (support-tests-openai-chunk '(:content "one"))))
           (head nil) (incomplete nil)))
  (:when (progn (setq head (pearl-provider-sse-parse-head text))
                (setq incomplete (pearl-provider-sse-parse-head
                                  (substring (support-tests-sse-head) 0 30)))))
  (:then (equal 200 (car head)))
  (:then (equal (support-tests-openai-chunk '(:content "one")) (cdr head)))
  (:then (null incomplete)))

;; A tool call is one object the endpoint sends in fragments: the id and the
;; name once, the arguments in pieces.  The parser turns the fragment into a
;; call the loop can concatenate, with the index that identifies it.
(ert-gwt-deftest
  (:given ((frame (support-tests-openai-tool-call 0 "call_1" "read" ""))
           (event nil)))
  (:when (setq event (pearl-provider-sse-parse-event (string-trim frame))))
  (:then (eq 'tool-call (plist-get event :type)))
  (:then (equal 0 (plist-get event :index)))
  (:then (equal "call_1" (plist-get event :id)))
  (:then (equal "read" (plist-get event :name))))

;; The state concatenates the fragments by index, so two calls the endpoint
;; interleaves come out as two complete calls in the order the model made
;; them, not as one merged object.
(ert-gwt-deftest
  (:given ((state nil)
           (frames (list (support-tests-openai-tool-call 0 "call_1" "read" "")
                         (support-tests-openai-tool-call 1 "call_2" "edit" "")
                         (support-tests-openai-tool-call 0 nil nil "{\"path\":")
                         (support-tests-openai-tool-call 1 nil nil "{\"file\":")
                         (support-tests-openai-tool-call 0 nil nil "\"a.el\"}")
                         (support-tests-openai-tool-call 1 nil nil "\"b.el\"}")))))
  (:when (dolist (frame frames)
           (setq state (pearl-provider-sse-tool-call-delta
                        state (pearl-provider-sse-parse-event (string-trim frame))))))
  (:then (equal "read" (plist-get (car state) :name)))
  (:then (equal "{\"path\":\"a.el\"}" (plist-get (car state) :arguments)))
  (:then (equal "edit" (plist-get (cadr state) :name)))
  (:then (equal "{\"file\":\"b.el\"}" (plist-get (cadr state) :arguments))))

;; `finish_reason' is how a turn ends, in the vocabulary the loop uses: a
;; plain stop is the end, a truncated turn says so, and a vendor's own reason
;; becomes an error rather than a turn that runs half of its tool calls.
(ert-gwt-deftest
  (:given ((frames (list (support-tests-openai-chunk '(:content "") "stop")
                         (support-tests-openai-chunk '(:content "") "length")
                         (support-tests-openai-chunk '(:content "") "tool_calls")
                         (support-tests-openai-chunk '(:content "") "content_filter")))
           (events nil)))
  (:when (setq events (mapcar (lambda (frame)
                                (pearl-provider-sse-parse-event (string-trim frame)))
                              frames)))
  (:then (eq 'stop (plist-get (nth 0 events) :type)))
  (:then (eq 'end (plist-get (nth 0 events) :reason)))
  (:then (eq 'length (plist-get (nth 1 events) :reason)))
  (:then (eq 'tool-calls (plist-get (nth 2 events) :reason)))
  (:then (eq 'error (plist-get (nth 3 events) :reason))))

;; `data: [DONE]' terminates the body.  It is not a message, so nothing about
;; it reaches the console.
(ert-gwt-deftest
  (:given ((event nil)))
  (:when (setq event (pearl-provider-sse-parse-event (string-trim (support-tests-openai-done)))))
  (:then (eq 'done (plist-get event :type)))
  (:then (null (plist-get event :text)))
  (:then (null (plist-get event :reason))))

;; A chunk we do not understand is ignored, not fatal: the endpoint sends an
;; empty delta before the first token, vendors add fields, and a half-written
;; object is a bug in the stream rather than a reason to lose the turn.
(ert-gwt-deftest
  (:given ((bodies (list "{\"id\":\"x\",\"object\":\"chat.completion.chunk\",\"choices\":[]}"
                       "{\"id\":\"x\",\"vendor_thing\":{\"a\":1},\"choices\":[]}"
                       "{\"choices\":[{\"delta\":"))
           (events nil)))
  (:when (setq events (mapcar (lambda (body)
                                (pearl-provider-sse-parse-event
                                 (string-trim (support-tests-sse-frame body))))
                              bodies)))
  (:then (equal '(nil nil nil) events)))

;; The headers ask for a stream and carry the key as a bearer token; the body
;; carries the model, the messages, the declarations and the one flag that
;; makes the response a stream.
(ert-gwt-deftest
  (:given ((headers (pearl-provider-sse-headers "secret-key"))
           (request (pearl-contract-request
                     (list (pearl-contract-message 'user "go"))
                     :tools '((:name "read" :description "read a file"))))
           (body nil)))
  (:when (setq body (pearl-provider-sse-request-body request "some-model")))
  (:then (equal "Bearer secret-key" (alist-get "Authorization" headers nil nil #'equal)))
  (:then (equal "text/event-stream" (alist-get "Accept" headers nil nil #'equal)))
  (:then (equal "application/json" (alist-get "Content-Type" headers nil nil #'equal)))
  (:then (equal "some-model" (plist-get body :model)))
  (:then (equal "user" (plist-get (aref (plist-get body :messages) 0) :role)))
  (:then (equal "go" (plist-get (aref (plist-get body :messages) 0) :content)))
  (:then (eq t (plist-get body :stream)))
  (:then (equal "read" (plist-get (aref (plist-get body :tools) 0) :name))))

;; A request with no tools is a request with no tools key: an empty array is
;; a declaration the model is offered and cannot call.
(ert-gwt-deftest
  (:given ((request (pearl-contract-request
                     (list (pearl-contract-message 'user "go"))))
           (body nil)))
  (:when (setq body (pearl-provider-sse-request-body request "some-model")))
  (:then (equal "some-model" (plist-get body :model)))
  (:then (equal "go" (plist-get (aref (plist-get body :messages) 0) :content)))
  (:then (null (plist-get body :tools))))

;; The conversation survives the conversion in both directions: a message
;; goes into a payload and comes back as the message it was, so a story can
;; assert on what the model would have been told.
(ert-gwt-deftest
  (:given ((message (pearl-contract-message 'user "hello")) (back nil)))
  (:when (setq back (pearl-provider-sse-read-payload
                     (pearl-provider-sse-payload message))))
  (:then (eq 'user (plist-get back :role)))
  (:then (equal "hello" (pearl-contract-text back))))

;; A call is a payload the endpoint reads in its own spelling, and the trip
;; back keeps the id the result will be matched on.
(ert-gwt-deftest
  (:given ((call (pearl-contract-tool-call "call-1" "read" '((path . "pearl.el"))))
           (back nil)))
  (:when (setq back (pearl-provider-sse-read-payload
                     (pearl-provider-sse-payload call))))
  (:then (equal "call-1" (plist-get back :id)))
  (:then (equal "read" (plist-get back :name)))
  (:then (equal "pearl.el" (alist-get 'path (plist-get back :arguments)))))

;; The key is named where the user already keeps credentials: the environment
;; answers when nothing else does, and PEARL writes it nowhere.
(ert-gwt-deftest
  (:given ((pearl-provider-sse-key-cache nil) (key nil) (auth-sources nil))
          (setenv "PEARL_API_KEY" "sk-env"))
  (:when (setq key (pearl-provider-sse-api-key "env.example.com")))
  (:then (equal "sk-env" key))
  (:cleanup (setenv "PEARL_API_KEY" nil)))

;; A credential the user already configured wins over the environment, so a
;; one-off export does not silently replace the key their editor uses.
(ert-gwt-deftest
  (:given ((file (ert-gwt--temp-file "pearl-authinfo"))
           (pearl-provider-sse-key-cache nil) (key nil)
           (auth-sources (list file)))
          (with-temp-file file
            (insert "machine auth.example.com password sk-auth\n"))
          (setenv "PEARL_API_KEY" "sk-env"))
  (:when (setq key (pearl-provider-sse-api-key "auth.example.com")))
  (:then (equal "sk-auth" key))
  (:cleanup (setenv "PEARL_API_KEY" nil)))

;; Ask once.  A session that prompted per turn would tax every request for a
;; secret the user typed at the first one.
(ert-gwt-deftest
  (:given ((pearl-provider-sse-key-cache nil) (asked 0) (first nil) (second nil)
           (auth-sources nil))
          (setenv "PEARL_API_KEY" nil)
          (progn
            (setq pearl-provider-sse-read-passwd
                  (lambda (&rest _) (setq asked (1+ asked)) "sk-typed"))))
  (:when (progn (setq first (pearl-provider-sse-api-key "ask.example.com"))
                (setq second (pearl-provider-sse-api-key "ask.example.com"))))
  (:then (equal "sk-typed" first))
  (:then (equal "sk-typed" second))
  (:then (= 1 asked)))

;; Nowhere to look and nothing to type: the request fails before it is sent,
;; naming the host, rather than going out with an empty bearer.
(ert-gwt-deftest
  (:given ((pearl-provider-sse-key-cache nil) (condition nil)
           (auth-sources nil))
          (setenv "PEARL_API_KEY" nil)
          (progn (setq pearl-provider-sse-read-passwd (lambda (&rest _) nil))))
  (:when (setq condition (condition-case err
                             (progn (pearl-provider-sse-api-key "nowhere.example.com") nil)
                           (pearl-api-key-missing (car err)))))
  (:then (eq 'pearl-api-key-missing condition)))

;; The SSE client is a provider: one interface, two implementations, and a
;; plist that merely looks like a provider is not one.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-sse-client :base-url "http://127.0.0.1:1/v1" :api-key "k" :model "m"))
           (fake (pearl-provider-fake '((stop "end"))))
           (accepted nil)))
  (:when (setq accepted (pearl-provider-p provider)))
  (:then accepted)
  (:then (pearl-provider-p fake))
  (:then (not (pearl-provider-p '(:base-url "http://127.0.0.1:1/v1" :api-key "k")))))

;; A rejected key is a status the user reads, not a stream that ends in
;; silence: the request errors by name, and it is not asked again, because a
;; key that was just refused will be refused with the same 401.
(ert-gwt-deftest
  (:given ((connections 0)
           (server (support-tests-tcp-server
                    (lambda (client)
                      (setq connections (1+ connections))
                      (process-send-string
                       client (support-tests-http-response
                               "401 Unauthorized"
                               "{\"error\":{\"message\":\"bad key\"}}")))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse-client :base-url (format "http://127.0.0.1:%d/v1" port)
                                         :api-key "k" :model "m" :idle-timeout 5))
           (failure nil) (done 0)))
  (:when (progn (pearl-provider-request provider '(:messages nil) #'ignore
                                        :on-done (lambda () (setq done (1+ done)))
                                        :on-error (lambda (err) (setq failure err)))
                (support-tests-wait (lambda () failure))))
  (:then (eq 'http-error (plist-get failure :reason)))
  (:then (equal 401 (plist-get failure :status)))
  (:then (equal "bad key" (plist-get (plist-get failure :body) :message)))
  (:then (equal 1 connections))
  (:then (equal 1 done))
  (:cleanup (support-tests-tcp-close server)))

;; The endpoint is read off the URL, because the scheme is what decides the
;; transport: https is the only form that may carry a key.
(ert-gwt-deftest
  (:given ((endpoint nil)))
  (:when (setq endpoint (pearl-provider-sse-endpoint "https://api.openai.com/v1")))
  (:then (eq 'https (plist-get endpoint :scheme)))
  (:then (equal "api.openai.com" (plist-get endpoint :host)))
  (:then (equal 443 (plist-get endpoint :port)))
  (:then (equal "/v1" (plist-get endpoint :path))))

;; An explicit port wins over the scheme's default, which is what makes a
;; local server in a story reachable without a flag.
(ert-gwt-deftest
  (:given ((endpoint nil)))
  (:when (setq endpoint (pearl-provider-sse-endpoint "http://127.0.0.1:8080/v1")))
  (:then (eq 'http (plist-get endpoint :scheme)))
  (:then (equal "127.0.0.1" (plist-get endpoint :host)))
  (:then (equal 8080 (plist-get endpoint :port)))
  (:then (equal "/v1" (plist-get endpoint :path))))

;; No scheme is not a default to guess: picking http for a https endpoint is
;; how a key goes out in clear text, so the URL is refused instead.
(ert-gwt-deftest
  (:given ((condition nil)))
  (:when (setq condition (condition-case err
                             (progn (pearl-provider-sse-endpoint "api.openai.com/v1") nil)
                           (pearl-provider-invalid-url (car err)))))
  (:then (eq 'pearl-provider-invalid-url condition)))

;; A 503 is a node being replaced, not a wrong request: it is asked again, and
;; the caller is released once at the end rather than once per attempt.
(ert-gwt-deftest
  (:given ((connections 0) (seen nil) (done 0) (failure nil)
           (server (support-tests-tcp-server
                    (lambda (client)
                      (setq connections (1+ connections))
                      (if (= connections 1)
                          (process-send-string
                           client (support-tests-http-response
                                   "503 Service Unavailable"
                                   "{\"error\":{\"message\":\"replacing\"}}"
                                   '(("Retry-After" . "0"))))
                        (process-send-string
                         client (concat (support-tests-sse-head)
                                        (support-tests-openai-chunk '(:content "one") "stop")
                                        (support-tests-openai-done)))))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse-client :base-url (format "http://127.0.0.1:%d/v1" port)
                                         :api-key "k" :model "m" :idle-timeout 5))))
  (:when (progn (pearl-provider-request provider '(:messages nil)
                                        (lambda (event) (push event seen))
                                        :on-done (lambda () (setq done (1+ done)))
                                        :on-error (lambda (err) (setq failure err)))
                (support-tests-wait (lambda () (= 1 done)))))
  (:then (equal 2 connections))
  (:then (equal '("one")
                (delq nil (mapcar (lambda (event) (plist-get event :text))
                                  (reverse seen)))))
  (:then (null failure))
  (:cleanup (support-tests-tcp-close server)))

;; A 429 with no attempts left is the status the user reads, not a hang and
;; not a silent retry loop.
(ert-gwt-deftest
  (:given ((connections 0) (failure nil) (done 0)
           (server (support-tests-tcp-server
                    (lambda (client)
                      (setq connections (1+ connections))
                      (process-send-string
                       client (support-tests-http-response
                               "429 Too Many Requests"
                               "{\"error\":{\"message\":\"slow down\"}}"
                               '(("Retry-After" . "0")))))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse-client :base-url (format "http://127.0.0.1:%d/v1" port)
                                         :api-key "k" :model "m"
                                         :idle-timeout 5 :retry-limit 0))))
  (:when (progn (pearl-provider-request provider '(:messages nil) #'ignore
                                        :on-done (lambda () (setq done (1+ done)))
                                        :on-error (lambda (err) (setq failure err)))
                (support-tests-wait (lambda () failure))))
  (:then (eq 'http-error (plist-get failure :reason)))
  (:then (equal 429 (plist-get failure :status)))
  (:then (equal 1 connections))
  (:then (equal 1 done))
  (:cleanup (support-tests-tcp-close server)))

;; A body that closes before its terminator is a cut stream, not a short
;; answer: it is named, kept where it is, and never asked again, because the
;; second copy would render the same paragraph twice (ADR 18).
(ert-gwt-deftest
  (:given ((connections 0) (seen nil) (failure nil) (done 0)
           (server (support-tests-tcp-server
                    (lambda (client)
                      (setq connections (1+ connections))
                      (process-send-string
                       client (concat (support-tests-sse-head)
                                      (support-tests-openai-chunk '(:content "one"))))
                      (process-send-eof client))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse-client :base-url (format "http://127.0.0.1:%d/v1" port)
                                         :api-key "k" :model "m" :idle-timeout 5))))
  (:when (progn (pearl-provider-request provider '(:messages nil)
                                        (lambda (event) (push event seen))
                                        :on-done (lambda () (setq done (1+ done)))
                                        :on-error (lambda (err) (setq failure err)))
                (support-tests-wait (lambda () failure))))
  (:then (eq 'truncated (plist-get failure :reason)))
  (:then (equal 1 connections))
  (:then (equal "one" (plist-get (car seen) :text)))
  (:then (equal 1 done))
  (:cleanup (support-tests-tcp-close server)))

;; A connection that stops producing is abandoned by name: the idle limit
;; fires, and the user sees which limit it was.
(ert-gwt-deftest
  (:given ((server (support-tests-tcp-server
                    (lambda (client)
                      (process-send-string client (support-tests-sse-head)))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse-client :base-url (format "http://127.0.0.1:%d/v1" port)
                                         :api-key "k" :model "m" :idle-timeout 0.2))
           (failure nil)))
  (:when (progn (pearl-provider-request provider '(:messages nil) #'ignore
                                        :on-done #'ignore
                                        :on-error (lambda (err) (setq failure err)))
                (support-tests-wait (lambda () failure))))
  (:then (eq 'idle-timeout (plist-get failure :reason)))
  (:cleanup (support-tests-tcp-close server)))

;; A stream that produces too much is abandoned by name: the byte limit fires
;; even though the stream never went idle, and the head is not what filled it.
(ert-gwt-deftest
  (:given ((server (support-tests-tcp-server
                    (lambda (client)
                      (process-send-string client (support-tests-sse-head))
                      (process-send-string
                       client (support-tests-openai-chunk
                               (list :content (make-string 5000 ?x)))))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse-client :base-url (format "http://127.0.0.1:%d/v1" port)
                                         :api-key "k" :model "m"
                                         :idle-timeout 5 :byte-limit 1000))
           (failure nil)))
  (:when (progn (pearl-provider-request provider '(:messages nil) #'ignore
                                        :on-done #'ignore
                                        :on-error (lambda (err) (setq failure err)))
                (support-tests-wait (lambda () failure))))
  (:then (eq 'byte-limit (plist-get failure :reason)))
  (:cleanup (support-tests-tcp-close server)))

;; A stream that never stops producing is abandoned by name: the total limit
;; fires while chunks keep arriving.
(ert-gwt-deftest
  (:given ((timer nil)
           (server (support-tests-tcp-server
                    (lambda (client)
                      (process-send-string client (support-tests-sse-head))
                      (setq timer (run-with-timer
                                   0 0.05
                                   (lambda ()
                                     (when (process-live-p client)
                                       (process-send-string
                                        client (support-tests-openai-chunk
                                                '(:content "x"))))))))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse-client :base-url (format "http://127.0.0.1:%d/v1" port)
                                         :api-key "k" :model "m"
                                         :idle-timeout 5 :total-timeout 0.3))
           (failure nil)))
  (:when (progn (pearl-provider-request provider '(:messages nil) #'ignore
                                        :on-done #'ignore
                                        :on-error (lambda (err) (setq failure err)))
                (support-tests-wait (lambda () failure))))
  (:then (eq 'total-timeout (plist-get failure :reason)))
  (:cleanup (when timer (cancel-timer timer)) (support-tests-tcp-close server)))

;; A message streams as it arrives: the first delta is delivered before the
;; second one is sent, both land in order, and `[DONE]' finishes the request
;; without waiting for the connection to close.
(ert-gwt-deftest
  (:given ((timer nil) (seen nil) (done nil) (early nil)
           (server (support-tests-tcp-server
                    (lambda (client)
                      (process-send-string
                       client (concat (support-tests-sse-head)
                                      (support-tests-openai-chunk '(:role "assistant" :content ""))
                                      (support-tests-openai-chunk '(:content "one"))))
                      (setq timer
                            (run-with-timer
                             0.4 nil
                             (lambda ()
                               (when (process-live-p client)
                                 (process-send-string
                                  client (concat (support-tests-openai-chunk '(:content "two") "stop")
                                                 (support-tests-openai-done))))))))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse-client :base-url (format "http://127.0.0.1:%d/v1" port)
                                         :api-key "k" :model "m" :idle-timeout 5))))
  (:when (progn (pearl-provider-request provider '(:messages nil)
                                        (lambda (event) (push event seen))
                                        :on-done (lambda () (setq done t))
                                        :on-error #'ignore)
                (support-tests-wait (lambda () seen) 2)
                (setq early (delq nil (mapcar (lambda (event) (plist-get event :text))
                                              (reverse seen))))
                (support-tests-wait (lambda () done) 3)))
  (:then (equal '("one") early))
  (:then (eq t done))
  (:then (equal '("one" "two")
                (delq nil (mapcar (lambda (event) (plist-get event :text))
                                  (reverse seen))))))

;; A late reply from an abandoned request is dropped, not rendered: the
;; cancel happened first, so nothing the server says afterwards arrives.
(ert-gwt-deftest
  (:given ((timer nil) (seen nil) (done nil)
           (server (support-tests-tcp-server
                    (lambda (client)
                      (process-send-string client (support-tests-sse-head))
                      (setq timer
                            (run-with-timer
                             0.3 nil
                             (lambda ()
                               (when (process-live-p client)
                                 (process-send-string
                                  client (support-tests-openai-chunk '(:content "late"))))))))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse-client :base-url (format "http://127.0.0.1:%d/v1" port)
                                         :api-key "k" :model "m" :idle-timeout 5))
           (handle nil)))
  (:when (progn (setq handle (pearl-provider-request provider '(:messages nil)
                                                     (lambda (event) (push event seen))
                                                     :on-done (lambda () (setq done t))
                                                     :on-error #'ignore))
                (pearl-provider-cancel provider handle)
                (support-tests-wait (lambda () done) 2)
                (sleep-for 0.4)))
  (:then done)
  (:then (null seen))
  (:cleanup (when timer (cancel-timer timer)) (support-tests-tcp-close server)))

;; A cancelled request keeps its promise: the chunk that arrives afterwards is
;; dropped rather than rendered into a turn the user has already left.
(ert-gwt-deftest
  (:given ((client nil)
           (server (support-tests-tcp-server
                    (lambda (c)
                      (setq client c)
                      (process-send-string c (support-tests-sse-head)))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse-client
                      :base-url (format "http://127.0.0.1:%d/v1" port)
                      :api-key "k" :model "m" :idle-timeout 5))
           (handle nil) (seen nil) (done 0)))
  (:when (progn
           (setq handle (pearl-provider-request
                         provider '(:messages nil)
                         (lambda (event) (push event seen))
                         :on-done (lambda () (setq done (1+ done)))))
           (support-tests-wait (lambda () client) 2)
           (pearl-provider-sse-cancel handle)
           (process-send-string client
                                (support-tests-openai-chunk '(:content "late")))
           (support-tests-wait (lambda () done) 0.4)))
  (:then (equal 0 done))
  (:then (null (seq-filter (lambda (event)
                             (equal "late" (plist-get event :text)))
                           seen)))
  (:cleanup (support-tests-tcp-close server)))

(provide 'provider-sse-tests)

;;; provider-sse-tests.el ends here
