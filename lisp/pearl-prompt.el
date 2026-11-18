;;; pearl-prompt.el --- the text the model is given  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The prompt is three kinds of text and one order, and this module is both.
;; What PEARL ships is a function of the tool loadout, so the prose cannot
;; offer a call the loop cannot make; what the user and the project wrote
;; arrives as sources the product read and the session stored, and this module
;; only renders them.
;;
;; It knows no file, no session and no buffer: its input is the sources and
;; the declarations, which is what lets a core story test a prompt without a
;; temporary directory, and what stops a second way in from being invented.
;; ADR 21 records why there is exactly one mechanism and not a seam beside it.
;;
;; Planned API
;;
;; TODO `pearl-prompt-default' (TOOLS) -> string
;;      The prompt PEARL ships.  It names the tools in TOOLS, one line each,
;;      and says nothing about a tool that is not there, so a prompt that
;;      offers a call the loop cannot make is impossible by construction.  It
;;      states the three things a schema cannot carry: an edit lands without
;;      per-hunk approval because undo is the net, an edit is anchored by exact
;;      text so the smallest change that compiles is the right one, and a write
;;      the turn's snapshot cannot cover will be confirmed, so a command that
;;      writes belongs in one call rather than many.  A constant, not a
;;      defcustom: advice on `pearl-prompt-sections' is how it is replaced or
;;      removed, at the price of carrying the whole text, because a prompt is
;;      per-session and a variable is not (ADR 21).
;;
;; TODO `pearl-prompt-sections' (SOURCES TOOLS) -> list of sections
;;      The one place the prompt's order is written down (ADR 20).  A section
;;      is `(:name NAME :path PATH :text TEXT)', and PATH is nil for the text
;;      PEARL ships.  The order is: `preamble' from `pearl-prompt-default'
;;      over TOOLS, then SOURCES in the order they were stored.  That is the
;;      whole list: there is no second way in (ADR 21), so an extension that
;;      needs text which is not in a file sets a source on the session before
;;      the next turn.  Nothing here reads a file.
;;
;; TODO `pearl-prompt' (SOURCES TOOLS) -> string
;;      Render the sections: the preamble untagged, every other section
;;      wrapped in a tag of its own name, and a section that came from a file
;;      naming that file in a `path' attribute, so the model can tell a rule
;;      written for this repository from one written for this user.

;;; Code:

(provide 'pearl-prompt)

;;; pearl-prompt.el ends here
