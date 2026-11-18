;;; session-jsonl-tests.el --- stories about the record on disk  -*- lexical-binding: t; -*-

;;; Commentary:

;; An appended entry has to still be there when Emacs is not, and a
;; half-written tail is a fact to report rather than to skip.

;;; Code:

(require 'support-tests)
(require 'pearl-session-jsonl)
(require 'pearl-contract)
(require 'ert-gwt)

(ert-gwt-deftest
  (:given ((path (ert-gwt--temp-file "pearl-session.jsonl")) (store (pearl-session-jsonl-open path :create t)) (entry (pearl-contract-entry 'message '(:content "hi"))) (back nil)))
  (:when (progn (pearl-session-jsonl-append store entry) (pearl-session-jsonl-close store) (setq store (pearl-session-jsonl-open path)) (setq back (pearl-session-jsonl-read store))))
  (:then (equal 1 (length back)))
  (:then (equal "hi" (plist-get (plist-get (car back) :data) :content))))

(ert-gwt-deftest
  (:given ((path (ert-gwt--temp-file "pearl-bad.jsonl")) (store nil)))
  (:when (progn (with-temp-file path (insert "{not json}\n")) (setq store (pearl-session-jsonl-open path))))
  (:then (should-error (pearl-session-jsonl-read store))))

;; A patch survives the trip to disk: the durable rollback of M9 reads
;; exactly the patch the tool wrote, not a re-derived one.
(ert-gwt-deftest
  (:given ((path (ert-gwt--temp-file "pearl-patch.jsonl"))
           (patch '(:tool "edit" :file "a.el" :old-text "a" :new-text "b"))
           (store (pearl-session-jsonl-open path :create t))
           (back nil)))
  (:when (progn (pearl-session-jsonl-append store (pearl-contract-entry 'tool '(:name "edit") nil patch)) (pearl-session-jsonl-close store) (setq back (pearl-session-jsonl-read (pearl-session-jsonl-open path)))))
  (:then (equal patch (plist-get (car back) :patch)))
  (:then (eq 'tool (plist-get (car back) :kind))))

;; Entries come back in file order: the history is the order it happened,
;; not the order a reader felt like returning.
(ert-gwt-deftest
  (:given ((path (ert-gwt--temp-file "pearl-order.jsonl")) (store (pearl-session-jsonl-open path :create t)) (back nil)))
  (:when (progn (pearl-session-jsonl-append store (pearl-contract-entry 'message '(:content "one"))) (pearl-session-jsonl-append store (pearl-contract-entry 'message '(:content "two"))) (pearl-session-jsonl-append store (pearl-contract-entry 'message '(:content "three"))) (pearl-session-jsonl-close store) (setq back (pearl-session-jsonl-read (pearl-session-jsonl-open path)))))
  (:then (equal '("one" "two" "three") (mapcar (lambda (entry) (plist-get (plist-get entry :data) :content)) back))))

;; A half-written tail is reported by line number: the user learns where
;; the record stopped, instead of a silent skip.
(ert-gwt-deftest
  (:given ((path (ert-gwt--temp-file "pearl-tail.jsonl")) (store nil) (err nil)))
  (:when (progn (with-temp-file path (insert "{\"kind\":\"message\",\"data\":{\"content\":\"ok\"}}\n" "{half written")) (setq store (pearl-session-jsonl-open path)) (setq err (condition-case e (pearl-session-jsonl-read store) (pearl-session-jsonl-corrupt e)))))
  (:then (eq 'pearl-session-jsonl-corrupt (car err)))
  (:then (string-match-p "2" (format "%S" (cdr err)))))

;; Close is idempotent: shutdown runs it twice, and the second run is
;; not an error.
(ert-gwt-deftest
  (:given ((path (ert-gwt--temp-file "pearl-close.jsonl")) (store (pearl-session-jsonl-open path :create t)) (second-close :not-called)))
  (:when (progn (pearl-session-jsonl-close store) (setq second-close (pearl-session-jsonl-close store))))
  (:then (null second-close)))

;; Where a session lives under a root is the adapter's decision, and the file
;; it names stays under that root, carries the session id and reads as a
;; record.
(ert-gwt-deftest
  (:given ((root (support-tests-temp-root)) (path nil)))
  (:when (setq path (pearl-session-jsonl-path-for root "story")))
  (:then (string-prefix-p (file-name-as-directory root) path))
  (:then (string-match-p "story" path))
  (:then (string-suffix-p ".jsonl" path)))

;; Two stores on one record are refused by name, and the refusal names the
;; holder: the lock has to be a durable fact about the file, not just a
;; variable in the process that opened it.
(ert-gwt-deftest
  (:given ((path (ert-gwt--temp-file "pearl-locked.jsonl"))
           (first (pearl-session-jsonl-open path :create t :lock t))
           (message nil)))
  (:when (setq message
               (condition-case err
                   (progn (pearl-session-jsonl-open path :create t :lock t) nil)
                 (pearl-session-locked (error-message-string err)))))
  (:then (string-match-p "pearl-locked" message))
  (:then (string-match-p (format "%d" (emacs-pid)) message))
  (:cleanup (pearl-session-jsonl-close first)))

;; Closing releases the lock: the same session can be opened again by the same
;; process, which is what reopening a console on its own record looks like.
(ert-gwt-deftest
  (:given ((path (ert-gwt--temp-file "pearl-relock.jsonl"))
           (first (pearl-session-jsonl-open path :create t :lock t))
           (second nil) (back nil)))
  (:when (progn (pearl-session-jsonl-append
                 first (pearl-contract-entry 'message '(:content "one")))
                (pearl-session-jsonl-close first)
                (setq second (pearl-session-jsonl-open path :lock t))
                (pearl-session-jsonl-append
                 second (pearl-contract-entry 'message '(:content "two")))
                (setq back (pearl-session-jsonl-read second))))
  (:then (equal '("one" "two")
                (mapcar (lambda (entry) (plist-get (plist-get entry :data) :content))
                        back)))
  (:cleanup (when second (pearl-session-jsonl-close second))))

;; A lock whose holder is gone is stale and is taken over once, so a killed
;; Emacs costs the next one a takeover and not an afternoon.
(ert-gwt-deftest
  (:given ((path (ert-gwt--temp-file "pearl-stale.jsonl"))
           (lock (concat path ".lock"))
           (store nil))
          (make-directory lock)
          (with-temp-file (expand-file-name "owner" lock)
            (insert "4194303")))
  (:when (setq store (pearl-session-jsonl-open path :create t :lock t)))
  (:then (string-match-p (format "\\b%d\\b" (emacs-pid))
                         (with-temp-buffer
                           (insert-file-contents (expand-file-name "owner" lock))
                           (buffer-string))))
  (:cleanup (when store (pearl-session-jsonl-close store))))

(provide 'session-jsonl-tests)

;;; session-jsonl-tests.el ends here
