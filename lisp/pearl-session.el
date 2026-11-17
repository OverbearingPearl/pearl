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
;; TODO `pearl-session-projection' (SESSION REQUEST) -> list of messages
;;      The pure function that owns what the model sees: walk the active
;;      branch, prepend the system prompt from `pearl-prompt-function', add
;;      the tool declarations, and fold tool output a later turn has already
;;      read: a tool entry with an assistant response after it on the branch
;;      projects as a short marker, never the full text, while the newest
;;      tool output stays whole.  The entry itself keeps everything.  It
;;      cannot add an entry, and compaction is a change to this function
;;      rather than to the tree.
;;
;; TODO `pearl-prompt-function' -> defvar, a function of no arguments
;;      Returns the system prompt string.  The default says what PEARL is
;;      and how its tools behave; an extension rebinds the variable, which
;;      is ADR 6's second seam.  Data, not behaviour: no registry, no
;;      discovery.
;;
;; TODO `pearl-session-patch' (ENTRY) -> patch or nil
;;      Only an entry that changed something outside the session carries one.
;;      A patch names the tool, the file, the text replaced and the text
;;      written, plus a hash where one side has no text (create, delete,
;;      rename).  The core records patches and never interprets them.
;;
;; TODO `pearl-session-rollback-plan' (SESSION &optional ENTRY) -> plan
;;      The patches to reverse, from the head back to ENTRY, in reverse order
;;      and scoped to one path.  Never a single entry out of the middle: a
;;      later patch on the same path was written against this one.  Applying
;;      the plan is the product's job.
;;
;; TODO `pearl-session-conflict-p' (PATCH CONTENT) -> boolean
;;      Non-nil when PATCH cannot be reversed against CONTENT, which is what
;;      makes a refused restore possible.  A patch whose written text is empty
;;      is always a conflict: an empty segment cannot be found uniquely.

;;; Code:

(provide 'pearl-session)

;;; pearl-session.el ends here
