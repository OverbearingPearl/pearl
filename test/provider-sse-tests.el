;;; provider-sse-tests.el --- stories about one stream  -*- lexical-binding: t; -*-

;;; Commentary:

;; A chunk boundary can fall anywhere, including inside an event, and a
;; dropped fragment looks exactly like a model that changed its mind.

;;; Code:

(require 'pearl-support-tests)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((partial "") (chunk (concat "data: one\n" "\n" "data: tw"))))
  (:when (pearl-provider-sse-parse-chunk partial chunk))
  (:then (equal "one" (plist-get (car (pearl-provider-sse-parse-chunk partial chunk)) :text))))

(ert-gwt-deftest
  (:given ((line "data: {\"id\":\"call-1\",\"name\":\"read\"}")))
  (:when (pearl-provider-sse-parse-event line))
  (:then (equal "read" (plist-get (pearl-provider-sse-parse-event line) :name))))

(ert-gwt-deftest
  (:given ((provider (pearl-provider-sse :base-url "http://127.0.0.1:1/v1" :api-key "k" :model "m" :idle-timeout 0.1)) (failure nil)))
  (:when (pearl-provider-request provider nil (function ignore) :on-done (lambda () nil) :on-error (lambda (err) (setq failure err))))
  (:then (equal (quote idle-timeout) (plist-get failure :reason))))
(provide 'provider-sse-tests)

;;; provider-sse-tests.el ends here
