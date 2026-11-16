;;; pearl-tool.el --- tool shape, dispatch and validation  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The shape a tool has to have, and the trust boundary it crosses.  Whatever
;; a model emits is unvalidated text in a data structure; this module is the
;; one place that turns it into arguments.  After that point no module checks
;; again, and a tool that fails returns a result marked as an error rather than
;; signalling: the loop must be able to carry on and show the model what went
;; wrong.
;;
;; Which tools exist is not decided here.  A tool is data, and the list is one
;; of the three seams an extension may replace (ADR 6).
;;
;; Planned API
;;
;; TODO `pearl-tool-define' (NAME DECLARATION RUN &optional VALIDATE) -> tool
;;      DECLARATION is the description and argument schema shown to the model,
;;      RUN is called with validated arguments and a context plist, VALIDATE is
;;      an optional predicate that runs before RUN.
;;
;; TODO `pearl-tool-list' (&optional REGISTRY) -> list of tools
;;      The default registry is the variable an extension rebinds; there is no
;;      discovery step.
;;
;; TODO `pearl-tool-declarations' (TOOLS) -> list of declarations
;;      What the provider is told is available, in list order.
;;
;; TODO `pearl-tool-validate' (TOOL ARGUMENTS) -> validated arguments
;;      Signals `pearl-tool-invalid' with the offending key.  Called once, at
;;      the boundary, and never again downstream.  Borrow: pi validates in one
;;      place for the same reason, pinned in ADR 8.
;;
;; TODO `pearl-tool-run' (TOOL ARGUMENTS CONTEXT) -> tool result
;;      Validate, call, and turn every outcome into a result through
;;      `pearl-contract-tool-result': a signal becomes an error result, a
;;      missing field becomes an error result, and an `escape' from the user
;;      becomes an aborted result.  Never a signal.
;;
;;      A successful result may carry `:patch', which is present only for a
;;      tool that wrote outside the session.  It is opaque here: the tool
;;      layer carries it, the loop records it, the session never interprets
;;      it.  A result without one is a conversational entry and cannot be
;;      rolled back.
;;
;; TODO `pearl-tool-error-result' (TOOL MESSAGE) -> tool result
;;      The error shape, so every failure reads the same in a transcript.
;;
;; TODO `pearl-tool-truncate' (TEXT &key HEAD TAIL LIMIT) -> (TEXT . TRUNCATEDP)
;;      Output bounding with an explicit marker, because a model that cannot
;;      tell it saw a fragment will reason about the fragment as if it were the
;;      whole.  Borrow: pi's head and tail windows, pinned in ADR 8.

;;; Code:

(provide 'pearl-tool)

;;; pearl-tool.el ends here
