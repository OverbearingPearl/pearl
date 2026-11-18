;;; pearl-session.el --- entries, branches and projection  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The record of what happened, and the only module that decides what the model
;; reads next.  Entries are immutable and only appended; an entry carries the
;; id of its parent, which makes the session a tree and not a log.  The path
;; from the root to the head is the active branch, and it is what the
;; projection turns into context.
;;
;; This module is in the core, next to the loop, because PEARL's promise is
;; that an applied change can be taken back, and a guarantee that depends on
;; the layer above remembering to record something is a convention rather than
;; a guarantee.  ADR 3 records the argument and its price.  Storing entries is
;; an adapter's job, and this module never learns a file name.
;;
;; Planned API
;;
;; TODO `pearl-session-new' (&optional ROOT-ENTRY) -> session
;;      A session with no head yet, or one whose head is ROOT-ENTRY.
;;
;; TODO `pearl-session-append' (SESSION ENTRY) -> entry
;;      Set the entry's parent to the current head and make it the head.  The
;;      returned entry is the stored one, so a caller never holds a copy.
;;
;; TODO `pearl-session-entries' (SESSION) -> list of entries in append order
;; TODO `pearl-session-head' (SESSION) -> entry or nil
;; TODO `pearl-session-parent' (SESSION ENTRY) -> entry or nil
;; TODO `pearl-session-set-head' (SESSION ENTRY) -> nil
;;      Move to an existing entry.  Branches stay in the session and leave the
;;      active branch; nothing is deleted.
;; TODO `pearl-session-branch' (SESSION) -> list of entries, root first
;;
;; TODO `pearl-session-projection' (SESSION REQUEST) -> list of messages The
;;      pure function that owns what the model sees: walk the active branch,
;;      prepend `(pearl-session-prompt SESSION TOOLS)' over the declarations
;;      the request carries, add the tool declarations, and fold tool
;;      output a later turn has already read: a
;;      tool entry with an assistant response after it on the branch projects as
;;      a short marker, never the full text, while the newest tool output stays
;;      whole.  The entry itself keeps everything.  It cannot add an entry, and
;;      compaction is a change to this function rather than to the tree: the
;;      entries a `compaction' entry covers do not appear, and its summary
;;      appears once, at that entry's position (ADR 27).
;;
;;      The projection is append-only in the sense the provider's cache needs:
;;      the messages it produced for a branch never change unless an entry is
;;      appended, and a fold never alters anything before the entry it folds.
;;      Recomputing the head every turn would be correct and would cost full
;;      price on every request, which is why this is a rule and not a tuning
;;      choice.
;;
;; TODO `pearl-session-default-prompt' (TOOLS) -> string
;;      The prompt PEARL ships.  It names the tools in TOOLS, one line each,
;;      and says nothing about a tool that is not there, so a prompt that
;;      offers a call the loop cannot make is impossible by construction.  It
;;      states the three things a schema cannot carry: an edit lands without
;;      per-hunk approval because undo is the net, an edit is anchored by exact
;;      text so the smallest change that compiles is the right one, and a write
;;      the turn's snapshot cannot cover will be confirmed, so a command that
;;      writes belongs in one call rather than many.  A constant, not a
;;      defcustom: advice on `pearl-session-prompt-sections' is how it is
;;      replaced or removed, at the price of carrying the whole text, because
;;      a prompt is per-session and a variable is not (ADR 21).
;;
;; TODO `pearl-session-prompt-sections' (SESSION TOOLS) -> list of sections
;;      The one place the prompt's order is written down (ADR 20).  A section
;;      is `(:name NAME :path PATH :text TEXT)', and PATH is nil for the text
;;      PEARL ships.  The order is: `preamble' from
;;      `pearl-session-default-prompt' over TOOLS, then the sources the
;;      product stored, in the order it stored them.  That is the whole list:
;;      there is no second way in (ADR 21), so an extension that needs text
;;      which is not in a file sets a source on the session before the next
;;      turn.  Nothing here reads a file.
;;
;; TODO `pearl-session-prompt' (SESSION TOOLS) -> string
;;      Render SESSION's sections: the preamble untagged, every other section
;;      wrapped in a tag of its own name, and a section that came from a file
;;      naming that file in a `path' attribute, so the model can tell a rule
;;      written for this repository from one written for this user.
;;
;; TODO `pearl-session-set-prompt-sources' (SESSION SOURCES) -> SESSION
;;      Store what the product read, as a list of `(:name :path :text)'.  The
;;      session holds them and never loads them, which is why a core story
;;      can test a prompt without a temporary directory.
;;
;; TODO `pearl-session-compact' (SESSION SUMMARY &key KEEP) -> entry
;;      Append a `compaction' entry whose data carries the entries it covers
;;      and the summary of them.  Nothing is removed: the covered entries stay
;;      on the branch, so replay, rollback and audit read the same record they
;;      did before (ADR 27).  SUMMARY is supplied by the caller, because a
;;      summary is a model's answer and this function only records it; the core
;;      keeps its promise of being testable with no network.  KEEP is how much
;;      of the tail stays unfolded, as entries rather than tokens: a count is
;;      predictable, and a token cut would move under the reader.
;;
;; TODO `pearl-session-expand' (SESSION ENTRY) -> list of entries
;;      The entries a compaction covers, in branch order.  Expanding is a read
;;      and not a projection mode: what the user asks to see is data that was
;;      never deleted, which is why "undo the compaction" needs no special
;;      operation.
;;
;; TODO `pearl-session-token-estimate' (SESSION) -> number
;;      A cheap estimate of the projected size, so the status line can show it
;;      and a threshold can be checked without a model.  Characters over four,
;;      which is what every harness in this space does; it is an estimate and
;;      the contract says so, because a number that looks exact gets trusted
;;      as if it were.
;;
;; TODO `pearl-session-compact-needed-p' (SESSION WINDOW) -> nil or `threshold'
;;      Nil when the projection fits under WINDOW less the reserve, otherwise
;;      `threshold'.  It answers a question rather than running a policy, and
;;      it returns a symbol so a second trigger has somewhere to say its name.
;;      The adapter's overflow answer is the product's own trigger and does not
;;      pass through here (ADR 27).
;;
;; TODO `pearl-session-covers' (ENTRY) -> list of ids or nil
;;      The entries a compaction covers, by id, or nil when ENTRY is not one.
;;      The projection and the console both need the answer and neither should
;;      know the field names (ADR 27).
;;
;; TODO `pearl-session-patch' (ENTRY) -> patch or nil
;;      Only an entry that changed something outside the session carries one.
;;      A patch names the tool, the file, the text replaced and the text
;;      written, plus a hash where one side has no text (create, delete,
;;      rename).  The core records patches and never interprets them.
;;
;; TODO `pearl-session-rollback-plan' (SESSION &optional ENTRY) -> plan
;;      What takes the session back to ENTRY, in reverse order, one path per
;;      step: `(:patch PATCH)' where a patch can reverse the change,
;;      `(:restore PATH)' where only the turn's snapshot can, and
;;      `(:preimage PATH)' where the snapshot never covered the path and the
;;      tool call's own pre-image is what takes it back (ADR 23, ADR 25).
;;      Never a single entry out of the middle: a later patch on the same
;;      path was written against this
;;      one.  A turn with nothing attributable contributes no step, because a
;;      step that cannot name what it reverted is a lie the console repeats.
;;      Applying the plan is the product's job.
;;
;; TODO `pearl-session-conflict-p' (PATCH CONTENT) -> boolean
;;      Non-nil when PATCH cannot be reversed against CONTENT, which is what
;;      makes a refused restore possible.  A patch whose written text is empty
;;      is always a conflict: an empty segment cannot be found uniquely.

;;; Code:

(provide 'pearl-session)

;;; pearl-session.el ends here
