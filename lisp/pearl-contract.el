;;; pearl-contract.el --- message, tool and entry shapes  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The vocabulary the rest of the package passes around, and the only place a
;; shape is written down.  Every other module may assume these shapes and none
;; of them may invent a second one: the loop, the session and the provider all
;; speak this.  What a provider wants on the wire is deliberately absent: an
;; adapter writes it at the edge (ADR 7), because a field name an endpoint chose
;; is that endpoint's, and a core that spells it has to change when a vendor
;; renames it.
;;
;; Nothing here does IO, and that is the point: a contract that can be built in
;; a test with no buffer, no process and no network is what lets the core stay
;; testable on its own.
;;
;; Planned API
;;
;; TODO `pearl-contract-message' (ROLE CONTENT &optional TIMESTAMP) -> message
;;      ROLE is a symbol, CONTENT a string or a list of content parts.  The
;;      returned plist is what the loop emits and the session stores.
;;
;; TODO `pearl-contract-message-p' (OBJECT) -> boolean
;;      Non-nil when OBJECT has a role and content field of the right types.
;;      Used at the wire boundary, never as a substitute for validation.
;;
;; TODO `pearl-contract-text' (MESSAGE) -> string
;;      The concatenated text of MESSAGE, for the console, the projection and
;;      the token estimate.  Signals when MESSAGE holds no text at all.
;;
;; TODO `pearl-contract-tool-call' (ID NAME ARGUMENTS) -> tool call
;;      ARGUMENTS is the alist `pearl-contract-parse-arguments' produced from
;;      the text the endpoint sent, unvalidated.  The tool layer validates;
;;      this constructor only names the fields.
;;
;; TODO `pearl-contract-parse-arguments' (STRING) -> alist
;;      The one place the wire's `arguments' text becomes a shape.  The
;;      endpoint sends an object, or the empty string for a call that takes
;;      none; both become an alist, and the empty string becomes nil so the
;;      common case does not look like a failure.  Anything else signals
;;      `pearl-tool-arguments-invalid' naming the fault: a truncated object, a
;;      JSON array, a scalar.  The loop turns that into the call's error
;;      result, because a model that emitted half an object is a turn that
;;      continues, not one that dies in the editor's command loop.
;;
;; TODO `pearl-contract-tool-result' (CALL RESULT &optional ERRORP)
;;      -> tool result, carrying the call id it answers so a transcript can be
;;      replayed without relying on position.
;;
;; TODO `pearl-contract-tool-declaration' (TOOL) -> declaration
;;      The description and argument schema a provider needs in order to offer
;;      TOOL to a model.
;;
;; TODO `pearl-contract-request' (MESSAGES &key TOOLS MODEL THINKING) -> request
;;      What a provider is asked, in the vocabulary this module owns: the
;;      messages the projection produced, the declarations of the tools that
;;      are loaded, the model to use and the thinking level.  The shape is here
;;      because three modules used to build a request and each decided a key of
;;      its own; a request is data, so it is the contract's.
;;
;; TODO `pearl-contract-request-p' (OBJECT) -> boolean
;;      Non-nil when OBJECT is a request: a list of messages under `:messages',
;;      which is the one field a provider cannot do without.  The provider
;;      checks it once, so a caller that forgot the conversation learns that
;;      before a connection is opened.
;;
;; TODO `pearl-contract-entry' (KIND DATA &optional PARENT PATCH) -> entry
;;      One immutable session record.  KIND is one of `message', `tool',
;;      `model', `compaction', `undo'; PATCH is nil for anything that changed
;;      nothing outside the session.  An undo is another entry, not a
;;      rewrite: a rollback appends an `undo' entry naming what it took back.
;;
;;      Every entry carries an `:id', and `:parent' names the entry it
;;      continues or nil at the root, because the record is a tree rather than
;;      a log.  The ids are what lets a compaction name the entries it covers
;;      and survive the JSONL round trip, which is why they are part of the
;;      contract and not an implementation detail (ADR 27).
;;
;;      Every entry also carries a `:timestamp', seconds since the epoch,
;;      because a page that puts two sessions in one order needs one clock per
;;      entry and the file order of one session says nothing about another
;;      (ADR 30).  It is the entry's clock and not the file's mtime: a file
;;      mtime moves when anything is appended, and a copied session keeps the
;;      times its turns happened at.

;;; Code:

(provide 'pearl-contract)

;;; pearl-contract.el ends here
