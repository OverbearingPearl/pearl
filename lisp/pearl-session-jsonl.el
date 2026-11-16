;;; pearl-session-jsonl.el --- append-only storage for a session  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; One JSON object per line, appended and never rewritten.  The format is the
;; adapter's business and nothing above it may assume a file: an entry in, a
;; line out, and the ability to read every line back into entries.
;;
;; Append-only is not an optimisation.  A session that can be rewritten is a
;; session whose history can disagree with what the user watched happen, and
;; the promise that a change can be taken back after a restart depends on the
;; record being the same one that was written.
;;
;; Planned API
;;
;; TODO `pearl-session-jsonl-open' (PATH &key CREATE) -> store
;;      A store is an opaque struct holding the path and the open handle.
;;      Signals `pearl-session-jsonl-corrupt' on a line that does not parse,
;;      naming the line number: a half-written tail is a fact to report, not
;;      something to skip quietly.
;;
;; TODO `pearl-session-jsonl-append' (STORE ENTRY) -> entry
;;      Write one line and flush.  Flushing per entry is what makes a killed
;;      Emacs lose at most the entry being written.
;;
;; TODO `pearl-session-jsonl-read' (STORE) -> list of entries
;;      Every entry in file order.  The caller rebuilds the tree by `parentId';
;;      this module does not know what a parent means.
;;
;; TODO `pearl-session-jsonl-close' (STORE) -> nil
;;      Close the handle.  Idempotent, because shutdown runs it twice.
;;
;; TODO `pearl-session-jsonl-path-for' (ROOT SESSION-ID) -> path
;;      Where a session lives under a project root.  The layout is an adapter
;;      decision and stays here.

;;; Code:

(provide 'pearl-session-jsonl)

;;; pearl-session-jsonl.el ends here
