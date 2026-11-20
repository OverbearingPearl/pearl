;;; pearl-context.el --- the page that says what the model is told  -*- lexical-binding: t; -*-

;; Copyright (C) 2026 OverbearingPearl
;; Author: OverbearingPearl <OverbearingPearl@outlook.com>
;; Assisted-by: Pi
;; URL: https://github.com/OverbearingPearl/pearl
;; SPDX-License-Identifier: MIT

;;; Commentary:

;; The console draws what happened.  A request is built from something else:
;; the prompt the session carries, the messages its projection keeps, and the
;; summary a compaction put in place of the turns it covered.  "Why did it say
;; that" is a question about the second thing, and no buffer answered it.
;;
;; The page is a projection and not a copy.  It reads the record every time it
;; is asked for, it holds no session of its own, and it writes nothing, so the
;; two rules it has to keep are cheap: it is read-only, and a turn a compaction
;; covered is absent from it rather than folded.  What it does not carry is the
;; count a provider reports, because no provider here parses one (ADR 17 keeps
;; the wire's vocabulary below this page); the estimate per section is the same
;; arithmetic `pearl-compaction-estimate' spends its budget with.
;;
;; The order is the provider's order, and that is the point of the page: the
;; prompt is first because the request puts it first, and a section's place on
;; the page is its place in the request.

;;; Code:

(require 'pearl-compaction)
(require 'pearl-session)

(defconst pearl-context-buffer-format "*pearl context: %s*"
  "The name of a page, named after the console it belongs to.")

(defvar-local pearl-context--session nil
  "The session this page projects.")

(defvar-local pearl-context--tools nil
  "The tool declarations the prompt on this page names.")

(defun pearl-context--summaries (session)
  "Return the summary texts SESSION's compaction entries carry.
The projection carries a summary as a system message and says nothing about
where it came from, so the record is what tells a summary apart from the
prompt: the two are the same shape on the wire."
  (let ((summaries nil))
    (dolist (entry (pearl-session-branch session) summaries)
      (when (pearl-compaction-covers entry)
        (push (plist-get (plist-get entry :data) :summary) summaries)))))

(defun pearl-context-sections (session tools)
  "Return SESSION's projection as sections, given TOOLS.
Each section is a plist (:title :text :tokens), in the order the provider
receives them.  The first system message is the prompt, and a later system
message whose text is one of the branch's summaries is a compaction."
  (let ((summaries (pearl-context--summaries session))
        (first t)
        (sections nil))
    (dolist (message (pearl-session-projection session tools))
      (let* ((role (plist-get message :role))
             (text (or (plist-get message :content) ""))
             (title (cond ((member text summaries) "compaction summary")
                          ((and first (eq role 'system)) "system prompt")
                          (t (format "%s" (or role 'system))))))
        (push (list :title title
                    :text text
                    :tokens (pearl-compaction-tokens text))
              sections)
        (setq first nil)))
    (nreverse sections)))

(defun pearl-context--insert-section (section)
  "Insert SECTION, its title line and its text."
  (insert (propertize (format "%s  %d tokens"
                              (plist-get section :title)
                              (plist-get section :tokens))
                      'face 'bold)
          "\n")
  (insert (plist-get section :text) "\n\n"))

(defun pearl-context--draw (buffer)
  "Erase BUFFER and draw its sections from the record again."
  (with-current-buffer buffer
    (let ((inhibit-read-only t))
      (erase-buffer)
      (dolist (section (pearl-context-sections pearl-context--session
                                               pearl-context--tools))
        (pearl-context--insert-section section))
      (goto-char (point-min))))
  buffer)

(defun pearl-context-buffer (session tools suffix)
  "Return the page showing what SESSION's next request is built from.
TOOLS are the declarations the prompt names, and SUFFIX names the page after
the console it belongs to.  Asking for a page that is already open draws it
again rather than making a second one, because the record moved on and the
page did not."
  (let ((buffer (get-buffer-create (format pearl-context-buffer-format suffix))))
    (with-current-buffer buffer
      ;; The mode first: `kill-all-local-variables' is what a derived mode
      ;; runs, and a slot set before it is a slot the page does not have.
      (unless (derived-mode-p 'pearl-context-mode)
        (pearl-context-mode))
      (setq pearl-context--session session
            pearl-context--tools tools))
    (pearl-context--draw buffer)))

(defun pearl-context-revert (&rest _)
  "Draw the page again from the record it projects.
It is what `g' runs, through `revert-buffer-function': a page left open while
a turn lands is one key away from the truth instead of being a buffer the
user has to close and reopen."
  (pearl-context--draw (current-buffer)))

(define-derived-mode pearl-context-mode special-mode "Pearl-Context"
  "The major mode of a PEARL context page.
The page is a projection of the session record, so it is read-only, it owns
no session, and nothing typed into it can change what the model reads.  `g'
draws it again from the record."
  (setq-local revert-buffer-function #'pearl-context-revert))

(provide 'pearl-context)

;;; pearl-context.el ends here
