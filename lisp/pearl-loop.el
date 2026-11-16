;;; pearl-loop.el --- the double loop and its two queues  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The control flow, and the reason this package exists.  An inner loop runs
;; while there are tool calls or steering messages; an outer loop re-enters it
;; when follow-up messages are waiting.  The two channels differ in when they
;; may arrive, not in what they contain, and getting that wrong splits a turn
;; in half.
;;
;; The loop knows nothing about buffers, files or provider names.  It receives
;; a provider, a tool list, a session and an emitter, and it hands back what
;; happened; a surface can be replaced without the loop noticing.
;;
;; Every wait is a callback, and callbacks arrive from process sentinels.  The
;; three rules in ADR 10 are implemented here and nowhere else: defer each step
;; to a zero-delay timer, check the run's generation before each dispatch, and
;; report a dead buffer instead of dropping the chain.
;;
;; Planned API
;;
;; TODO `pearl-loop-new' (&key PROVIDER TOOLS SESSION EMIT ON-ERROR) -> loop
;;      PROVIDER answers requests, TOOLS is the executable tool list, SESSION
;;      is the record the projection reads.  TOOLS may change between turns;
;;      the change is declared to the model as a difference, per ADR 8.
;;
;; TODO `pearl-loop-submit' (LOOP PROMPT) -> run id
;;      Admit one user message and start a run.  Signals `pearl-loop-busy'
;;      when a run is in flight; the caller queues instead of interrupting.
;;
;; TODO `pearl-loop-steer' (LOOP MESSAGE) -> nil
;;      Deliver MESSAGE after the current turn's tool calls and before the
;;      next request.  The only channel that may arrive mid-run.
;;
;; TODO `pearl-loop-follow-up' (LOOP MESSAGE) -> nil
;;      Deliver MESSAGE when the loop would otherwise stop.  It re-enters the
;;      inner loop as a new outer iteration and never splits a turn.
;;
;; TODO `pearl-loop-abort' (LOOP) -> list of messages handed back
;;      Stop the run, keep the tool results already produced, and return the
;;      queue to the caller.  Borrow: pi's abort semantics, pinned in ADR 8.
;;
;; TODO `pearl-loop-busy-p' (LOOP) -> boolean
;; TODO `pearl-loop-queued' (LOOP &optional KIND) -> list of messages
;;      KIND is `steering', `follow-up', or nil for both in delivery order.
;;
;; TODO `pearl-loop--dispatch' (LOOP STEP) -> nil
;;      Schedule STEP on a zero-delay timer when the generation still matches.
;;      Borrow: scalpel's deferred-dispatch loop, `scalpel-agent-run', which is
;;      where the three rules in ADR 10 were worked out; the generation
;;      counter and the killed-buffer report are both its.  This comment is
;;      scaffolding: the code that lands here replaces it, and the record in
;;      ADR 10 says what survived.
;;
;; TODO `pearl-loop--turn' (LOOP) -> nil
;;      One assistant response plus the tool calls it makes.  A `length' stop
;;      fails every tool call of the turn instead of running truncated ones,
;;      and a failed tool call is a result, never a signal.  Borrow: pi's
;;      runLoop, pinned in ADR 8.

;;; Code:

(provide 'pearl-loop)

;;; pearl-loop.el ends here
