;;; turn-tests.el --- stories about one response, assembled  -*- lexical-binding: t; -*-

;;; Commentary:

;; The provider sends pieces and the session wants a message.  These stories
;; are about the place in between: what a fragment merges into, what a turn
;; that cannot be parsed becomes, and which reason ended it.

;;; Code:

(require 'support-tests)
(require 'pearl-turn)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((turn (pearl-turn-new :id "m1"))
           (deltas '((:type text :text "Hel") (:type text :text "lo")))))
  (:when (mapc (lambda (event) (pearl-turn-feed turn event)) deltas))
  (:then (equal "Hello" (pearl-contract-text (pearl-turn-message turn))))
  (:then (eq 'assistant (plist-get (pearl-turn-message turn) :role))))

(ert-gwt-deftest
  (:given ((turn (pearl-turn-new))
           (deltas '((:type tool-call :index 0 :id "c1" :name "read"
                      :arguments "")
                     (:type tool-call :index 0 :arguments "{\"path\":")
                     (:type tool-call :index 0 :arguments "\"a.el\"}")))))
  (:when (mapc (lambda (event) (pearl-turn-feed turn event)) deltas))
  (:then (= 1 (length (pearl-turn-calls turn))))
  (:then (equal "c1" (plist-get (car (pearl-turn-calls turn)) :id)))
  (:then (equal "a.el"
                (alist-get 'path
                           (plist-get (car (pearl-turn-calls turn))
                                      :arguments)))))

(ert-gwt-deftest
  (:given ((turn (pearl-turn-new))
           (deltas '((:type tool-call :index 0 :id "c1" :name "read"
                      :arguments "{\"path\":")
                     (:type stop :reason length)))))
  (:when (mapc (lambda (event) (pearl-turn-feed turn event)) deltas))
  (:then (equal "{\"path\":"
                (plist-get (car (pearl-turn-calls turn)) :arguments)))
  (:then (should-error (pearl-contract-parse-arguments
                        (plist-get (car (pearl-turn-calls turn))
                                   :arguments)))))

(ert-gwt-deftest
  (:given ((turn (pearl-turn-new))))
  (:when (pearl-turn-feed turn '(:type stop :reason length)))
  (:then (eq 'length (pearl-turn-stop turn)))
  (:then (pearl-turn-complete-p turn)))

(ert-gwt-deftest
  (:given ((turn (pearl-turn-new))
           (deltas '((:type stop :reason length) (:type stop :reason end)))))
  (:when (mapc (lambda (event) (pearl-turn-feed turn event)) deltas))
  (:then (eq 'length (pearl-turn-stop turn))))

(ert-gwt-deftest
  (:given ((turn (pearl-turn-new))))
  (:when (pearl-turn-feed turn '(:type done)))
  (:then (null (pearl-turn-message turn)))
  (:then (null (pearl-turn-calls turn)))
  (:then (eq 'end (pearl-turn-stop turn)))
  (:then (pearl-turn-complete-p turn)))

(provide 'turn-tests)

;;; turn-tests.el ends here
