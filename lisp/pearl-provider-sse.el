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
;; Three limits are part of the interface rather than a detail: an idle limit
;; (a connection that stops producing), a total limit (a stream that never
;; stops producing), and a byte limit (a stream that produces too much).  A
;; request that exceeds one of them is abandoned by name, so a user sees which
;; limit fired.
;;
;; Planned API
;;
;; TODO `pearl-provider-sse' (&key BASE-URL API-KEY MODEL IDLE-TIMEOUT
;;      TOTAL-TIMEOUT BYTE-LIMIT) -> provider
;;      Builds a provider that satisfies `pearl-provider-p'.  Defaults live in
;;      one `defcustom' block, because these are the numbers a user changes.
;;
;; TODO `pearl-provider-sse-headers' (API-KEY) -> alist
;;      The request headers, including the accept header that asks for a
;;      stream.  Authorization is built here and not logged anywhere.
;;
;; TODO `pearl-provider-sse-request-body' (REQUEST MODEL) -> plist
;;      Wire messages, declarations and the thinking level, in the shape the
;;      endpoint expects.  Borrow: pi's per-API request builders, of which this
;;      takes one; ADR 7 says why only one.
;;
;; TODO `pearl-provider-sse-parse-chunk' (PARTIAL CHUNK) -> (EVENTS . REST)
;;      Split a network chunk into complete SSE events and the incomplete tail.
;;      A chunk boundary can fall anywhere, including inside a UTF-8 sequence;
;;      that is the reason this is a function with a test rather than a line
;;      inside a filter.
;;
;; TODO `pearl-provider-sse-parse-event' (STRING) -> event plist or nil
;;      One event into `(:type text :text …)', `(:type tool-call …)' and the
;;      stop reason.  An unknown event is ignored rather than fatal, because
;;      vendors add them.
;;
;; TODO `pearl-provider-sse-tool-call-delta' (STATE DELTA) -> STATE
;;      Concatenate a partial tool call across events.  A tool call arrives as
;;      name first, then argument fragments; a parser that assumes one event
;;      produces a truncated JSON object and a model that appears to lie.
;;
;; TODO `pearl-provider-sse-cancel' (HANDLE) -> nil
;;      Delete the process and mark the request abandoned, so a late event is
;;      dropped instead of rendered.

;;; Code:

(provide 'pearl-provider-sse)

;;; pearl-provider-sse.el ends here
