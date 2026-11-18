;;; pearl-turn.el --- one response, assembled from deltas  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; A provider answers in pieces: a text delta, a fragment of a tool call's
;; arguments, a stop reason.  A turn is what assembles those pieces into the
;; message and the calls they spell, and it is a module of its own because the
;; alternative is a loop that is also a stream parser, which is a loop that
;; cannot be tested without one.
;
;; The pieces are not messages.  An endpoint opens a call with an index and a
;; name and then sends its arguments in fragments, and it need not repeat the
;; index a reader expects; the turn is the thing that remembers, so nothing
;; above it has to.  What goes in is a provider event, which is the provider's
;; vocabulary; what comes out is a message and a list of calls, which are the
;; contract's (ADR 26 keeps rendering out of both).
;;
;; Planned API
;;
;; `pearl-turn-new' (&key ID) -> turn
;;      An accumulator for one assistant response.  ID is the message id the
;;      console renders under, so a redraw replaces a message instead of
;;      appending the same one twice; nil leaves the turn to number its own.
;;
;; `pearl-turn-feed' (TURN EVENT) -> nil
;;      Take one provider event.  `(:type text :text STRING)' concatenates
;;      onto the response; `(:type tool-call :index N :id ID :name NAME
;;      :arguments TEXT)' merges by N, because the endpoint sends a call in
;;      fragments and the fragments of one call arrive in order; `(:type stop
;;      :reason SYMBOL)' is recorded once, and a second one is ignored rather
;;      than overwriting the first, because the first is what ended the
;;      response; `(:type done)' marks the body ended cleanly, which is only a
;;      reason when no `stop' arrived.  An event carrying an index the turn has
;;      not seen starts a new call, in the order the endpoint opened them.
;;
;; `pearl-turn-message' (TURN) -> message or nil
;;      The assistant message the response accumulated, as
;;      `pearl-contract-message' builds one, or nil when it carried no text and
;;      no call: an empty message is not something a transcript should record.
;;
;; `pearl-turn-calls' (TURN) -> list of tool calls
;;      The calls in the order the endpoint opened them, with their arguments
;;      parsed by `pearl-contract-parse-arguments'.  A call whose text does not
;;      parse keeps that text under `:arguments' and is not signalled here: the
;;      loop fails that one call by name and the turn continues (ADR 8), and a
;;      module that assembles is not where a turn dies.
;;
;; `pearl-turn-stop' (TURN) -> symbol or nil
;;      `end', `length', `error' or `truncated', or nil while the response is
;;      still arriving.  `length' means the model was cut off mid-response,
;;      which is a fact the loop acts on and the console shows.
;;
;; `pearl-turn-complete-p' (TURN) -> boolean
;;      Non-nil once a stop reason arrived.  Everything the endpoint sent
;;      before it is the response; a connection that closed without one is a
;;      truncated turn and not a short one (ADR 18), which is why this asks the
;;      turn rather than the process.

;;; Code:

(require 'cl-lib)
(require 'seq)
(require 'pearl-contract)

(defvar pearl-turn--sequence 0
  "How many turns this Emacs has made, for the ones nameless by choice.")

(cl-defstruct (pearl-turn (:constructor pearl-turn--create))
  "One assistant response, and the pieces it is being assembled from.

`raw' is a list in the order the endpoint opened the calls, not in the
order the last fragment arrived, because a transcript reads in the order
the model asked for things.  Each element keeps its argument text until
it is asked for parsed arguments: a fragment that does not parse is the
loop's business to fail, not this module's."
  id (text "") (raw nil) (stop nil))

(defun pearl-turn-new (&rest keys)
  "An accumulator for one response, over KEYS.

`:id' is the message id a console renders under, so a redraw replaces a
message instead of appending it twice; nil leaves the turn to number its
own, which is what a turn nobody watches chooses."
  (pearl-turn--create
   :id (or (plist-get keys :id)
           (format "turn-%d" (cl-incf pearl-turn--sequence)))))

(defun pearl-turn--call-at (turn index)
  "The call TURN opened at INDEX, or nil."
  (seq-find (lambda (call) (equal index (plist-get call :index)))
            (pearl-turn-raw turn)))

(defun pearl-turn--merge-call (turn event)
  "Merge the tool-call EVENT into TURN, by its index."
  (let* ((index (plist-get event :index))
         (call (pearl-turn--call-at turn index)))
    (if (null call)
        (setf (pearl-turn-raw turn)
              (append (pearl-turn-raw turn)
                      (list (list :index index
                                  :id (plist-get event :id)
                                  :name (plist-get event :name)
                                  :arguments (or (plist-get event :arguments) "")))))
      (let ((old (plist-get call :arguments))
            (text (plist-get event :arguments)))
        (when text
          (setcdr call (plist-put (cdr call) :arguments
                                  (if (and (stringp old) (stringp text))
                                      (concat old text)
                                    text))))))))

(defun pearl-turn-feed (turn event)
  "Take one provider EVENT into TURN."
  (pcase (plist-get event :type)
    ('text
     (setf (pearl-turn-text turn)
           (concat (pearl-turn-text turn) (or (plist-get event :text) ""))))
    ('tool-call (pearl-turn--merge-call turn event))
    ('stop
     (unless (pearl-turn-stop turn)
       (setf (pearl-turn-stop turn) (plist-get event :reason))))
    ('done
     (unless (pearl-turn-stop turn)
       (setf (pearl-turn-stop turn) 'end))))
  nil)

(defun pearl-turn-message (turn)
  "The assistant message TURN accumulated, or nil when it carried nothing."
  (let ((calls (pearl-turn-raw turn)))
    (unless (and (equal "" (pearl-turn-text turn)) (null calls))
      (pearl-contract-message 'assistant (pearl-turn-text turn)))))

(defun pearl-turn--arguments (call)
  "The parsed arguments of CALL, or its text when they do not parse."
  (let ((text (or (plist-get call :arguments) "")))
    (condition-case nil
        (pearl-contract-parse-arguments text)
      (error text))))

(defun pearl-turn-calls (turn)
  "Return the tool call TURN opened, in order, with the arguments parsed."
  (mapcar (lambda (call)
            (list :id (plist-get call :id)
                  :name (plist-get call :name)
                  :arguments (pearl-turn--arguments call)))
          (pearl-turn-raw turn)))

(provide 'pearl-turn)

;;; pearl-turn.el ends here
