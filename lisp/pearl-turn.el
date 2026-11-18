;;; pearl-turn.el --- one response, assembled from deltas  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; A provider answers in pieces: a text delta, a fragment of a tool call's
;; arguments, a stop reason.  A turn is what assembles those pieces into the
;; message and the calls they spell, and it is a module of its own because the
;; alternative is a loop that is also a stream parser, which is a loop that
;; cannot be tested without one.
;
;; The pieces are not messages.  An endpoint opens a call with an index and a
;; name and then sends its arguments in fragments, and it need not repeat the
;; index a reader expects; the turn is the thing that remembers, so nothing
;; above it has to.  What goes in is a provider event, which is the provider's
;; vocabulary; what comes out is a message and a list of calls, which are the
;; contract's (ADR 26 keeps rendering out of both).
;;
;; Planned API
;;
;; TODO `pearl-turn-new' (&key ID) -> turn
;;      An accumulator for one assistant response.  ID is the message id the
;;      console renders under, so a redraw replaces a message instead of
;;      appending the same one twice; nil leaves the turn to number its own.
;;
;; TODO `pearl-turn-feed' (TURN EVENT) -> nil
;;      Take one provider event.  `(:type text :text STRING)' concatenates
;;      onto the response; `(:type tool-call :index N :id ID :name NAME
;;      :arguments TEXT)' merges by N, because the endpoint sends a call in
;;      fragments and the fragments of one call arrive in order; `(:type stop
;;      :reason SYMBOL)' is recorded once, and a second one is ignored rather
;;      than overwriting the first, because the first is what ended the
;;      response; `(:type done)' marks the body ended cleanly, which is only a
;;      reason when no `stop' arrived.  An event carrying an index the turn has
;;      not seen starts a new call, in the order the endpoint opened them.
;;
;; TODO `pearl-turn-message' (TURN) -> message or nil
;;      The assistant message the response accumulated, as
;;      `pearl-contract-message' builds one, or nil when it carried no text and
;;      no call: an empty message is not something a transcript should record.
;;
;; TODO `pearl-turn-calls' (TURN) -> list of tool calls
;;      The calls in the order the endpoint opened them, with their arguments
;;      parsed by `pearl-contract-parse-arguments'.  A call whose text does not
;;      parse keeps that text under `:arguments' and is not signalled here: the
;;      loop fails that one call by name and the turn continues (ADR 8), and a
;;      module that assembles is not where a turn dies.
;;
;; TODO `pearl-turn-stop' (TURN) -> symbol or nil
;;      `end', `length', `error' or `truncated', or nil while the response is
;;      still arriving.  `length' means the model was cut off mid-response,
;;      which is a fact the loop acts on and the console shows.
;;
;; TODO `pearl-turn-complete-p' (TURN) -> boolean
;;      Non-nil once a stop reason arrived.  Everything the endpoint sent
;;      before it is the response; a connection that closed without one is a
;;      truncated turn and not a short one (ADR 18), which is why this asks the
;;      turn rather than the process.

;;; Code:

(provide 'pearl-turn)

;;; pearl-turn.el ends here
