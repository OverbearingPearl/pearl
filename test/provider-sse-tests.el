;;; provider-sse-tests.el --- stories about one stream  -*- lexical-binding: t; -*-

;;; Commentary:

;; A chunk boundary can fall anywhere, including inside an event, and a
;; dropped fragment looks exactly like a model that changed its mind.

;;; Code:

(require 'support-tests)
(require 'pearl-provider-sse)
(require 'pearl-provider)
(require 'ert-gwt)

;; A chunk carries complete events and an incomplete tail; the tail is
;; returned, not dropped, because the next chunk finishes it.
(ert-gwt-deftest
  (:given ((partial "") (chunk (concat "data: {\"type\":\"text\",\"text\":\"one\"}\n\n" "data: tw")) (parsed nil)))
  (:when (setq parsed (pearl-provider-sse-parse-chunk partial chunk)))
  (:then (equal "one" (plist-get (car (car parsed)) :text)))
  (:then (equal "data: tw" (cdr parsed))))

;; A boundary inside an event: the first half parses to nothing, and the
;; second half completes the event the first half started.
(ert-gwt-deftest
  (:given ((first-half "data: {\"type\":\"text\",\"te") (second-half "xt\":\"one\"}\n\n") (parsed nil) (finished nil)))
  (:when (progn (setq parsed (pearl-provider-sse-parse-chunk "" first-half)) (setq finished (pearl-provider-sse-parse-chunk (cdr parsed) second-half))))
  (:then (null (car parsed)))
  (:then (equal "one" (plist-get (car (car finished)) :text)))
  (:then (equal "" (cdr finished))))

(ert-gwt-deftest
  (:given ((line "data: {\"type\":\"tool-call\",\"id\":\"call-1\",\"name\":\"read\"}")))
  (:when (pearl-provider-sse-parse-event line))
  (:then (equal "read" (plist-get (pearl-provider-sse-parse-event line) :name)))
  (:then (equal "call-1" (plist-get (pearl-provider-sse-parse-event line) :id))))

;; A vendor adds an event we do not know: it is ignored, not fatal.
(ert-gwt-deftest
  (:given ((line "data: {\"type\":\"vendor-invented-this\"}")))
  (:when (pearl-provider-sse-parse-event line))
  (:then (null (pearl-provider-sse-parse-event line))))

;; A tool call arrives as name first, then argument fragments; the state
;; concatenates them instead of parsing each fragment as a whole object.
(ert-gwt-deftest
  (:given ((state nil)
           (name-delta '(:type tool-call :id "c1" :name "read"))
           (fragment-one '(:type tool-call :id "c1" :arguments "{\"path\":"))
           (fragment-two '(:type tool-call :id "c1" :arguments "\"a.el\"}"))))
  (:when (progn (setq state (pearl-provider-sse-tool-call-delta state name-delta)) (setq state (pearl-provider-sse-tool-call-delta state fragment-one)) (setq state (pearl-provider-sse-tool-call-delta state fragment-two))))
  (:then (equal "read" (plist-get state :name)))
  (:then (equal "{\"path\":\"a.el\"}" (plist-get state :arguments))))

;; The headers ask for a stream and carry the key; the body carries the
;; model and the messages, in the shape the endpoint expects.
(ert-gwt-deftest
  (:given ((headers (pearl-provider-sse-headers "secret-key"))))
  (:when headers)
  (:then (string-match-p "secret-key" (alist-get "Authorization" headers nil nil #'equal)))
  (:then (equal "text/event-stream" (alist-get "Accept" headers nil nil #'equal))))

(ert-gwt-deftest
  (:given ((request '(:messages ((:role "user" :content "go")))) (body nil)))
  (:when (setq body (pearl-provider-sse-request-body request "some-model")))
  (:then (equal "some-model" (plist-get body :model)))
  (:then (plist-get body :messages)))

;; The SSE client is a provider: one interface, two implementations.
(ert-gwt-deftest
  (:given ((provider (pearl-provider-sse :base-url "http://127.0.0.1:1/v1" :api-key "k" :model "m"))))
  (:when (pearl-provider-p provider))
  (:then (pearl-provider-p provider)))

;; A connection that stops producing is abandoned by name: the idle limit
;; fires, and the user sees which limit it was.
(ert-gwt-deftest
  (:given ((server (support-tests-tcp-server (lambda (_client) nil)))
           (port (process-contact server :service))
           (provider (pearl-provider-sse :base-url (format "http://127.0.0.1:%d/v1" port) :api-key "k" :model "m" :idle-timeout 0.2))
           (failure nil)))
  (:when (progn (pearl-provider-request provider '(:messages nil) #'ignore :on-done #'ignore :on-error (lambda (err) (setq failure err))) (support-tests-wait (lambda () failure))))
  (:then (eq 'idle-timeout (plist-get failure :reason)))
  (:cleanup (support-tests-tcp-close server)))

;; A stream that produces too much is abandoned by name: the byte limit
;; fires even though the stream never went idle.
(ert-gwt-deftest
  (:given ((server (support-tests-tcp-server (lambda (client) (process-send-string client (support-tests-sse-frame (concat "{\"type\":\"text\",\"text\":\"" (make-string 5000 ?x) "\"}"))))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse :base-url (format "http://127.0.0.1:%d/v1" port) :api-key "k" :model "m" :idle-timeout 5 :byte-limit 1000))
           (failure nil)))
  (:when (progn (pearl-provider-request provider '(:messages nil) #'ignore :on-done #'ignore :on-error (lambda (err) (setq failure err))) (support-tests-wait (lambda () failure))))
  (:then (eq 'byte-limit (plist-get failure :reason)))
  (:cleanup (support-tests-tcp-close server)))

;; A stream that never stops producing is abandoned by name: the total
;; limit fires while chunks keep arriving.
(ert-gwt-deftest
  (:given ((timer nil)
           (server (support-tests-tcp-server (lambda (client) (setq timer (run-with-timer 0 0.05 (lambda () (when (process-live-p client) (process-send-string client (support-tests-sse-frame "{\"type\":\"text\",\"text\":\"x\"}")))))))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse :base-url (format "http://127.0.0.1:%d/v1" port) :api-key "k" :model "m" :idle-timeout 5 :total-timeout 0.3))
           (failure nil)))
  (:when (progn (pearl-provider-request provider '(:messages nil) #'ignore :on-done #'ignore :on-error (lambda (err) (setq failure err))) (support-tests-wait (lambda () failure))))
  (:then (eq 'total-timeout (plist-get failure :reason)))
  (:cleanup (when timer (cancel-timer timer)) (support-tests-tcp-close server)))

;; A message streams as it arrives: the first delta is delivered before
;; the second one is sent, and both land in order.
(ert-gwt-deftest
  (:given ((timer nil) (seen nil) (done nil)
           (server (support-tests-tcp-server (lambda (client) (process-send-string client (support-tests-sse-frame "{\"type\":\"text\",\"text\":\"one\"}")) (setq timer (run-with-timer 0.4 nil (lambda () (when (process-live-p client) (process-send-string client (concat (support-tests-sse-frame "{\"type\":\"text\",\"text\":\"two\"}") (support-tests-sse-frame "{\"type\":\"stop\",\"reason\":\"end\"}"))))))))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse :base-url (format "http://127.0.0.1:%d/v1" port) :api-key "k" :model "m" :idle-timeout 5))))
  (:when (progn (pearl-provider-request provider '(:messages nil) (lambda (event) (push event seen)) :on-done (lambda () (setq done t)) :on-error #'ignore) (support-tests-wait (lambda () seen) 2)))
  (:then (equal "one" (plist-get (car seen) :text)))
  (:then (null (cdr seen)))
  (:then (support-tests-wait (lambda () done) 3))
  (:then (equal '("one" "two") (mapcar (lambda (event) (plist-get event :text)) (seq-filter (lambda (event) (plist-get event :text)) (nreverse seen)))))
  (:cleanup (when timer (cancel-timer timer)) (support-tests-tcp-close server)))

;; A late reply from an abandoned request is dropped, not rendered: the
;; cancel happened first, so nothing the server says afterwards arrives.
(ert-gwt-deftest
  (:given ((timer nil) (seen nil) (done nil)
           (server (support-tests-tcp-server (lambda (client) (setq timer (run-with-timer 0.3 nil (lambda () (when (process-live-p client) (process-send-string client (support-tests-sse-frame "{\"type\":\"text\",\"text\":\"late\"}")))))))))
           (port (process-contact server :service))
           (provider (pearl-provider-sse :base-url (format "http://127.0.0.1:%d/v1" port) :api-key "k" :model "m" :idle-timeout 5))
           (handle nil)))
  (:when (progn (setq handle (pearl-provider-request provider '(:messages nil) (lambda (event) (push event seen)) :on-done (lambda () (setq done t)) :on-error #'ignore)) (pearl-provider-cancel provider handle) (support-tests-wait (lambda () done) 2) (sleep-for 0.4)))
  (:then done)
  (:then (null seen))
  (:cleanup (when timer (cancel-timer timer)) (support-tests-tcp-close server)))

(provide 'provider-sse-tests)

;;; provider-sse-tests.el ends here
