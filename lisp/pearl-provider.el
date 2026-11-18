;;; pearl-provider.el --- the request interface and its fake  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; One interface, and the fake that proves it is an interface.  A provider
;; answers a request with events: text deltas, tool-call deltas, a stop reason.
;; It knows nothing about the loop, and the loop knows nothing about HTTP.
;;
;; There is no catalog, no auth layer and no adapter per vendor; ADR 7 records
;; what that costs.  The second implementation is deliberately a fake, because
;; an interface used only once is a guess, and an interface used by a test is
;; the shape the second vendor will get.
;;
;; Planned API
;;
;; `pearl-provider-request' (PROVIDER REQUEST EMIT &key ON-DONE ON-ERROR)
;;      -> handle
;;      REQUEST is what `pearl-contract-request' builds: the messages the
;;      session projected, the declarations of the loaded tools, the model and
;;      the thinking level.  EMIT is called with each provider event as it
;;      arrives.
;;      ON-DONE runs exactly once, on success and on failure, and never
;;      between attempts: it is the callback that releases the caller.
;;      ON-ERROR receives a condition, never a string.  A provider may retry a
;;      transient failure on its own, but not after it has emitted an event,
;;      and a body that ends before its terminator is an error rather than a
;;      short turn (ADR 18).  The reasons are symbols a surface can name:
;;      `http-error', `tls-error', `idle-timeout', `total-timeout',
;;      `byte-limit', `truncated'.  The call returns immediately: a provider
;;      that blocks would freeze the editor.
;;
;; `pearl-provider-cancel' (PROVIDER HANDLE) -> nil
;;      Ask for the request to stop.  A reply that arrives after the cancel is
;;      dropped rather than rendered, and the cancel is idempotent.
;;
;; `pearl-provider-fake' (SCRIPT &rest OPTIONS) -> provider
;;      SCRIPT is a list of steps: `(text "…")', `(tool-call ID NAME ARGS)',
;;      `(stop "end"|"length"|"error")', `(delay SECONDS)'.  OPTIONS may set
;;      `:fail-with' or `:hang'.  This is how a whole turn becomes data.
;;      ARGS is the object as an alist and is serialized into the text the
;;      endpoint sends, because that is what the wire carries; a string is
;;      passed through unchanged, which is how a story produces a call whose
;;      arguments do not parse.
;;
;; `pearl-provider-fake-requests' (PROVIDER) -> list of requests
;;      The requests the fake was asked, in call order.  A story asserts
;;      on what the model would have been told, not only on what it
;;      answered: steering position and tool-loadout differences are
;;      properties of the request, not of the reply.
;;
;; `pearl-provider-p' (OBJECT) -> boolean
;;      Non-nil for anything implementing the interface, so a caller can
;;      replace a provider with a fake in one place.

;;; Code:

(require 'cl-lib)
(require 'pearl-contract)

(define-error 'pearl-provider-invalid "A request this package cannot make")

(define-error 'pearl-provider-invalid-url
  "The endpoint URL is not one a key may be sent to"
  'pearl-provider-invalid)

(define-error 'pearl-api-key-missing
  "No API key was found for this host"
  'pearl-provider-invalid)

(cl-defstruct (pearl-provider--fake (:constructor pearl-provider--fake-create))
  script options requests cancelled)

(defun pearl-provider-p (object)
  "Non-nil when OBJECT implements the provider interface.

A provider is a fake, or a plist tagged `:pearl-provider' by the adapter
that built it: a plist that merely has the right keys is a request, not
an implementation."
  (or (pearl-provider--fake-p object)
      (and (consp object) (eq (car object) :pearl-provider))))

(defun pearl-provider-fake (script &rest options)
  "A provider that answers with SCRIPT, taking OPTIONS.

SCRIPT is a list of steps: `(text \"…\")', `(tool-call ID NAME ARGS)',
`(stop \"end\")' and `(delay SECONDS)'.  OPTIONS may carry `:fail-with'
or `:hang'.  A whole turn is data, which is how the loop is tested
without a network."
  (pearl-provider--fake-create :script (copy-sequence script)
                               :options options
                               :requests nil
                               :cancelled nil))

(defun pearl-provider-fake-requests (provider)
  "The requests PROVIDER was asked, in call order."
  (reverse (pearl-provider--fake-requests provider)))

(defun pearl-provider--event (step)
  "The event STEP delivers, or nil for a step that only waits."
  (pcase (car step)
    ('text (list :text (cadr step)))
    ('tool-call (list :id (nth 1 step)
                      :name (nth 2 step)
                      :arguments (nth 3 step)))
    ('stop (list :reason (cadr step)))))

(defun pearl-provider--schedule (provider script emit done error)
  "Book every step of SCRIPT, for PROVIDER, before the first one fires.

A timer that schedules the next timer does not fire while the editor is
waiting, so the whole script is on the clock from the start, with EMIT,
DONE and ERROR as the callbacks to run; a cancel is a flag they read
rather than a race."
  (let ((at 0))
    (dolist (step script)
      (if (eq (car step) 'delay)
          (setq at (+ at (cadr step)))
        (let ((event (pearl-provider--event step)))
          (run-at-time at nil
                       (lambda ()
                         (unless (pearl-provider--fake-cancelled provider)
                           (funcall emit event)))))))
    (run-at-time at nil
                 (lambda ()
                   (unless (pearl-provider--fake-cancelled provider)
                     (let ((failure (plist-get (pearl-provider--fake-options provider)
                                               :fail-with)))
                       (when failure
                         (funcall error (list failure "the fake was asked to fail"))))
                     (funcall done))))
    nil))

(defun pearl-provider-request (provider request emit &rest keys)
  "Ask PROVIDER for REQUEST, delivering events to EMIT.

KEYS carries `:on-done', run exactly once on success and on failure, and
`:on-error', which receives a condition.  The call returns immediately: a
provider that blocked would freeze the editor."
  (let ((done (or (plist-get keys :on-done) #'ignore))
        (error (or (plist-get keys :on-error) #'ignore)))
    (cond
     ((pearl-provider--fake-p provider)
      (push request (pearl-provider--fake-requests provider))
      (setf (pearl-provider--fake-cancelled provider) nil)
      (unless (plist-get (pearl-provider--fake-options provider) :hang)
        (pearl-provider--schedule provider (pearl-provider--fake-script provider)
                                  emit done error))
      provider)
     ((functionp (plist-get provider :request))
      (funcall (plist-get provider :request) provider request emit keys))
     (t (signal 'pearl-provider-invalid (list "not a provider" provider))))))

(defun pearl-provider-cancel (provider handle)
  "Ask PROVIDER to stop the request HANDLE names.

An adapter returns a handle of its own and is asked through it; the fake
answers by flagging itself."
  (cond
   ((pearl-provider--fake-p provider)
    (setf (pearl-provider--fake-cancelled provider) t))
   ((functionp (plist-get provider :cancel))
    (funcall (plist-get provider :cancel) provider handle)))
  nil)

(provide 'pearl-provider)

;;; pearl-provider.el ends here
