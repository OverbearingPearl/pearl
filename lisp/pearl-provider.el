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
;; TODO `pearl-provider-request' (PROVIDER REQUEST EMIT &key ON-DONE ON-ERROR)
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
;; TODO `pearl-provider-cancel' (PROVIDER HANDLE) -> nil
;;      Ask for the request to stop.  A reply that arrives after the cancel is
;;      dropped rather than rendered, and the cancel is idempotent.
;;
;; TODO `pearl-provider-fake' (SCRIPT &rest OPTIONS) -> provider
;;      SCRIPT is a list of steps: `(text "…")', `(tool-call ID NAME ARGS)',
;;      `(stop "end"|"length"|"error")', `(delay SECONDS)'.  OPTIONS may set
;;      `:fail-with' or `:hang'.  This is how a whole turn becomes data.
;;      ARGS is the object as an alist and is serialized into the text the
;;      endpoint sends, because that is what the wire carries; a string is
;;      passed through unchanged, which is how a story produces a call whose
;;      arguments do not parse.
;;
;; TODO `pearl-provider-fake-requests' (PROVIDER) -> list of requests
;;      The requests the fake was asked, in call order.  A story asserts
;;      on what the model would have been told, not only on what it
;;      answered: steering position and tool-loadout differences are
;;      properties of the request, not of the reply.
;;
;; TODO `pearl-provider-p' (OBJECT) -> boolean
;;      Non-nil for anything implementing the interface, so a caller can
;;      replace a provider with a fake in one place.

;;; Code:

(provide 'pearl-provider)

;;; pearl-provider.el ends here
