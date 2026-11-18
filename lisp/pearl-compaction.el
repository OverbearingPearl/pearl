;;; pearl-compaction.el --- what a summary covers and when one is due  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; Compaction is an entry, and its only effect is on the projection: the
;; entries it covers stop being read and stay in the record, so a summary is
;; reversible, attributable and replayable.  The rule itself is the session's,
;; because the projection is what changes; this module writes the entry and
;; answers whether one is due.
;;
;; It sits above the session because a summary is appended to one, and it never
;; calls a model: the text is supplied by the product, which is the layer that
;; knows how to reach one.  That is what keeps the core testable with no
;; network, and it is also why the trigger is a question rather than a policy.
;;
;; Planned API
;;
;; TODO `pearl-compaction-append' (SESSION SUMMARY &key KEEP) -> entry
;;      Append a `compaction' entry whose data carries the entries it covers
;;      and the summary of them.  Nothing is removed: the covered entries stay
;;      on the branch, so replay, rollback and audit read the same record they
;;      did before (ADR 27).  SUMMARY is supplied by the caller, because a
;;      summary is a model's answer and this function only records it.  KEEP is
;;      how much of the tail stays unfolded, as a token estimate over the
;;      projection and not as a count of entries: the constraint KEEP exists
;;      for is the provider's window, and one tool result can outgrow a hundred
;;      messages.  The cut is found once, walking back from the head until KEEP
;;      is spent, and then stored in the entry it writes, so the projection
;;      never re-derives it and a later append cannot move it under a reader.
;;      The cut never falls on a tool result, which belongs to its call.
;;
;;      The entry also carries what the range it folded had already lost: a
;;      compaction whose range contained a compaction is a tier above it, so a
;;      projection can say how many layers of summary stand behind what it
;;      reads.  A range that was expanded and folded again rewrites the entry it
;;      came from instead of nesting beside it, because two summaries of the
;;      same entries differ only in which one the reader sees.
;;
;;      It refuses rather than writes a no-op: a summary that is not smaller
;;      than the range it replaces, or a range whose entries carry too few
;;      characters to be worth a model call, appends nothing.  The refusal is
;;      the caller's to report, and the record is untouched until the entry is
;;      written.
;;
;; TODO `pearl-compaction-covered' (SESSION ENTRY) -> list of entries
;;      The entries a compaction covers, in branch order.  Expanding is a read
;;      and not a projection mode: what the user asks to see is data that was
;;      never deleted, which is why "undo the compaction" needs no special
;;      operation.
;;
;; TODO `pearl-compaction-estimate' (SESSION) -> number
;;      A cheap estimate of the projected size, so the status line can show it
;;      and a threshold can be checked without a model.  It counts a CJK
;;      character as one token and every other four as one, because that is
;;      what a tokeniser does to a language without spaces and the difference
;;      is a factor of two rather than a rounding error; a page in Chinese is
;;      not a quarter of a page in English.  It is an estimate and the contract
;;      says so, because a number that looks exact gets trusted as if it were.
;;
;; TODO `pearl-compaction-needed' (SESSION WINDOW) -> nil or `threshold'
;;      Nil when the projection fits under WINDOW less the reserve, otherwise
;;      `threshold'.  It answers a question rather than running a policy, and
;;      it returns a symbol so a second trigger has somewhere to say its name.
;;      The adapter's overflow answer is the product's own trigger and does not
;;      pass through here (ADR 27).
;;
;; TODO `pearl-compaction-covers' (ENTRY) -> list of ids or nil
;;      The entries a compaction covers, by id, or nil when ENTRY is not one.
;;      The projection and the console both need the answer and neither should
;;      know the field names (ADR 27).

;;; Code:

(provide 'pearl-compaction)

;;; pearl-compaction.el ends here
