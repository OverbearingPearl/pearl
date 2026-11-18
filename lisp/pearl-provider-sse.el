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
;; the model is saying.
;;
;; The key is resolved where the user already keeps credentials and is never
;; stored by PEARL (ADR 17), and it reaches the wire in one function.
;;
;; Planned API
;;
;; TODO `pearl-provider-sse' (&key BASE-URL API-KEY MODEL IDLE-TIMEOUT
;;      TOTAL-TIMEOUT BYTE-LIMIT) -> provider
;;      Builds a provider that satisfies `pearl-provider-p'.  BASE-URL is the
;;      prefix the endpoint ends, such as "https://api.openai.com/v1"; the
;;      request path is appended to it.  Defaults live in one `defcustom'
;;      block, because these are the numbers a user changes.  API-KEY nil
;;      means "resolve it", not "send none".  MODEL is the fallback for a
;;      request that carries no model: the console's choice travels in the
;;      request, so changing it mid-session changes the next request.
;;
;; TODO `pearl-provider-sse-api-key' (HOST) -> string
;;      The key for HOST, in the order ADR 17 fixes: `auth-source' for the
;;      host, then `PEARL_API_KEY', then one `read-passwd' prompt memoized for
;;      the session.  Signals `pearl-api-key-missing' naming HOST and the two
;;      lookups rather than sending an empty bearer.
;;
;; TODO `pearl-provider-sse-headers' (API-KEY) -> alist
;;      The request headers: `Authorization' as "Bearer KEY",
;;      `Content-Type: application/json', and the accept header that asks for
;;      a stream.  Authorization is built here and logged nowhere.
;;
;; TODO `pearl-provider-sse-request-body' (REQUEST MODEL) -> plist
;;      The chat-completions body for REQUEST: `:model', `:messages' as the
;;      wire roles and contents `pearl-contract-to-wire' produced, `:stream'
;;      true, and `:tools' from the declarations when the request carries
;;      any.  A vendor's reasoning field is not built here; it arrives as an
;;      ignored extra if the endpoint sends one.
;;
;; TODO `pearl-provider-sse-parse-head' (TEXT) -> (STATUS . REST) or nil
;;      Consume the status line and headers of the HTTP response and return
;;      the status code and whatever body arrived in the same chunk.  Nil
;;      means the head is still incomplete.  A status outside 2xx ends the
;;      request with `(:reason http-error :status STATUS :body ERROR)' once
;;      the body has arrived, because that is what a wrong key looks like.
;;      ERROR is the body decoded when it is JSON, so a caller reads the
;;      endpoint's own `message', and the raw text when it is not.
;;
;; TODO `pearl-provider-sse-parse-chunk' (PARTIAL CHUNK) -> (EVENTS . REST)
;;      Split a network chunk into complete SSE events and the incomplete tail.
;;      EVENTS is the list of what `pearl-provider-sse-parse-event' returned
;;      for each completed frame, in arrival order, and REST is the raw tail to
;;      prepend to the next chunk.  A blank line ends an event, and one event
;;      may carry several `data:' lines.  A chunk boundary can fall anywhere,
;;      including inside an event and inside a UTF-8 sequence; that is the
;;      reason this is a function with a test rather than a line in a filter.
;;
;; TODO `pearl-provider-sse-parse-event' (STRING) -> event plist or nil
;;      One OpenAI-compatible chunk into the loop's vocabulary:
;;      `(:type text :text …)' from `choices[0].delta.content',
;;      `(:type tool-call :index N :id … :name … :arguments …)' from
;;      `choices[0].delta.tool_calls', and `(:type stop :reason SYMBOL)' from
;;      `finish_reason', with `length' preserved because a truncated turn must
;;      not run its tool calls.  `data: [DONE]' is `(:type done)'.  An empty
;;      or unknown chunk, an unknown field and a malformed object are nil:
;;      vendors add events, and a parser that signals loses a working turn.
;;
;; TODO `pearl-provider-sse-tool-call-delta' (STATE DELTA) -> STATE
;;      Concatenate a partial tool call across deltas, keyed by the `index'
;;      the endpoint repeats in every fragment.  STATE is the list of calls in
;;      index order, each `(:index N :id … :name … :arguments …)', and a new
;;      index appends to it.  The id and the name arrive once and the
;;      `arguments' string arrives in pieces, so matching on id loses the
;;      second fragment and a parser that assumes one event produces a
;;      truncated JSON object and a model that appears to lie.
;;
;; TODO `pearl-provider-sse-cancel' (HANDLE) -> nil
;;      Delete the process and mark the request abandoned, so a late event is
;;      dropped instead of rendered.

;;; Code:

(provide 'pearl-provider-sse)

;;; pearl-provider-sse.el ends here
