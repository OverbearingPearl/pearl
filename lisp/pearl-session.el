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
;; Two things that used to live here live above it now, each for the reason it
;; could not be tested here: `pearl-prompt' renders text from sources without
;; knowing what a session is, and `pearl-compaction' writes the entry that
;; changes what the projection reads.  The projection itself stays, because it
;; is the one function that decides what the model sees next and the one place
;; the order of that is written down.
;;
;; Planned API
;;
;; TODO `pearl-session-new' (&optional ROOT-ENTRY) -> session
;;      A session with no head yet, or one whose head is ROOT-ENTRY.
;;
;; TODO `pearl-session-restore' (ENTRIES &optional HEAD) -> session
;;      The inverse of `pearl-session-jsonl-read': a session whose record is
;;      ENTRIES, in the order they were appended, and whose head is HEAD or,
;;      when that is nil, the last entry in ENTRIES.  This is what resuming is
;;      (ADR 29) - the store reads entries and this turns them back into the
;;      thing a console can continue, so the core still learns no file name.
;;      An entry whose parent is not in ENTRIES is not silently reattached: the
;;      record it came from is not the record it claims, and a session that
;;      guesses its own history cannot be replayed.  Entries are taken whole,
;;      so a restored session evaluates to the same projection as the one that
;;      wrote them.
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
;;      active branch; nothing is deleted.  ENTRY may be nil, meaning before the
;;      root: re-editing the first message is a sibling of it and not a
;;      replacement for it, and the append that follows writes the second
;;      attempt while the first stays readable (ADR 29).
;;
;; TODO `pearl-session-branch' (SESSION) -> list of entries, root first
;;      The path the head stands on, and the only thing the projection reads.
;;      The other branches stay in the session, unread until the head moves
;;      back onto one: rewinding is moving the head, and it is how a second
;;      attempt at the same point is written without erasing the first.
;;
;; TODO `pearl-session-projection' (SESSION TOOLS) -> list of messages The
;;      pure function that owns what the model sees: walk the active branch,
;;      prepend `(pearl-prompt (pearl-session-prompt-sources SESSION) TOOLS)'
;;      over the declarations TOOLS carry, and fold tool output a later turn has
;;      already read: a
;;      tool entry with an assistant response after it on the branch projects as
;;      a short marker, never the full text, while the newest tool output stays
;;      whole.  The entry itself keeps everything.  It cannot add an entry, and
;;      compaction is a change to this function rather than to the tree: the
;;      entries a `compaction' entry covers do not appear, and its summary
;;      appears once (ADR 27).
;;
;;      Where it appears is the earliest position the covered entries occupy,
;;      not the compaction entry's own, because the bytes a request is billed
;;      for are the ones before the fold point and the fold should move that
;;      point as far back as it honestly can.  Two places are refused: before
;;      the branch's first user entry, which providers reject when it goes
;;      missing, and between a tool call and the result that answers it, which
;;      no provider accepts on its own.  A rule about position is a rule about
;;      bytes, and it belongs in the function that emits them.
;;
;;      It takes TOOLS and not a request.  A request is what one provider is
;;      asked; the projection is what the branch says, and a function that read
;;      a request would make the record depend on the shape an adapter converts
;;      (ADR 7).  The order of the context is decided here and nowhere else:
;;      the prompt first, then the branch in append order, with the tool list
;;      as it stood at the entry that declared it.
;;
;;      The projection is latched in the sense the provider's cache needs: what
;;      it produced last time is kept and replayed, so a request that changes
;;      nothing above the fold point is billed as the same prefix.  The latch
;;      is refreshed by the events that must change it anyway - an append, a
;;      fold, a tool loadout that differs, a head that moved - and not by the
;;      passing of time or by a new estimate, which is the difference between a
;;      rule and a tuning knob.  Recomputing the head every turn would be
;;      correct and would cost full price on every request, which is why this
;;      is stated here rather than left to an implementer's instinct.
;;
;; TODO `pearl-session-prompt-sources' (SESSION) -> list of sources
;;      What the product read and this session stores, as a list of
;;      `(:name :path :text)'.  The session holds them and never loads them,
;;      which is why a core story can test a prompt without a temporary
;;      directory, and why `pearl-prompt' takes the list instead of a session.
;;
;; TODO `pearl-session-set-prompt-sources' (SESSION SOURCES) -> SESSION
;;      Replace the stored sources.  The product calls this when the prompt's
;;      inputs changed, before the next turn; there is no other way to add text
;;      to a prompt (ADR 21).
;;
;; TODO `pearl-session-patch' (ENTRY) -> patch or nil
;;      Only an entry that changed something outside the session carries one.
;;      A patch names the tool, the file, the text replaced and the text
;;      written, plus a hash where one side has no text (create, delete,
;;      rename).  The core records patches and never interprets them.

;;; Code:

(provide 'pearl-session)

;;; pearl-session.el ends here
