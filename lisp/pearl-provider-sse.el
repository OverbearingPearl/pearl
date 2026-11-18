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
;; TODO `pearl-provider-sse' (&key BASE-URL API-KEY MODEL IDLE-TIMEOUT
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
;; TODO `pearl-provider-sse-endpoint' (BASE-URL) -> plist
;;      The connection the URL names: `(:scheme https :host "api.example.com"
;;      :port 443 :path "/v1")'.  An explicit port wins over the scheme's
;;      default, and the path keeps its trailing form so the request path can
;;      be appended without guessing about slashes.  A URL with no scheme
;;      signals `pearl-provider-invalid-url': guessing between http and https
;;      is how a key goes out in clear text.
;;
;; TODO `pearl-base-url' -> defcustom, the endpoint prefix
;;      Every request goes here.  The default is the public
;;      OpenAI-compatible endpoint, and a user pointing PEARL at a compatible
;;      one sets this and nothing else.  `pearl-provider-sse' takes its
;;      BASE-URL from this when the argument is nil, so a console built with no
;;      provider of its own needs no Lisp to reach a different vendor.
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
;;      a stream.  Authorization is built here and logged nowhere.  The
;;      connection itself is opened by `pearl-provider-request' with
;;      `open-network-stream', its `:type' taken from the endpoint's scheme.
;;
;; TODO `pearl-provider-sse-payload' (MESSAGE) -> plist
;;      One message as the chat-completions body wants it: `:role' spelled the
;;      way the endpoint spells roles, and `:content' or `:tool_calls'
;;      depending on what the message carries.  The conversion lives in the
;;      adapter because the field names are the vendor's; a core that speaks
;;      them has to change when a vendor renames one (ADR 7).
;;
;; TODO `pearl-provider-sse-read-payload' (PAYLOAD) -> message
;;      The inverse, and the reason a story can assert on what the model would
;;      have been told: a test builds the body an endpoint receives and reads
;;      the messages back out of it.
;;
;; TODO `pearl-provider-sse-request-body' (REQUEST MODEL) -> plist
;;      The chat-completions body for REQUEST: `:model', `:messages' as the
;;      payloads `pearl-provider-sse-payload' writes, `:stream' true, and
;;      `:tools' from the declarations when the request carries any.  A
;;      vendor's reasoning field is not built here; it arrives as an ignored
;;      extra if the endpoint sends one.
;;
;; TODO `pearl-provider-sse-parse-head' (TEXT) -> (STATUS . REST) or nil
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
;;      not run its tool calls.  `data: [DONE]' is `(:type done)', and it is
;;      the only clean end of a body: a close before it is
;;      `(:reason truncated)', because a proxy reset mid-token renders as a
;;      short answer if the client trusts the end of file.  An empty or
;;      unknown chunk, an unknown field and a malformed object are nil:
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
