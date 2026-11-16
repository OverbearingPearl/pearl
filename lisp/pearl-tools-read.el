;;; pearl-tools-read.el --- the read tool  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; Read a file, or a window of one, and say which window it was.  The offsets
;; are part of the answer, not of the request alone: a model that is told it
;; saw lines 40 to 80 can ask for the next window, and one that is told
;; nothing asks again.
;;
;; A read never fails the turn.  A missing file, a binary file or an offset
;; past the end is a result the model can act on.
;;
;; Planned API
;;
;; TODO `pearl-tools-read-declaration' -> tool declaration
;;      `path', `offset' and `limit', with the same names and defaults as Pi's
;;      read tool (ADR 8), so a model trained on one works with the other.
;;
;; TODO `pearl-tools-read-run' (ARGUMENTS CONTEXT) -> tool result
;;      The returned text names the range it covers and the total line count,
;;      and the tool never signals: a missing file is an error result.
;;      CONTEXT supplies the project root, which is the only place a relative
;;      path is resolved against.
;;
;; TODO `pearl-tools-read-install' (REGISTRY) -> registry
;;      Add the tool to the list an extension may replace (ADR 6), and return
;;      the new list so installation reads as composition.

;;; Code:

(provide 'pearl-tools-read)

;;; pearl-tools-read.el ends here
