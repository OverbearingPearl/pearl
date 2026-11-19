;;; compaction-tests.el --- stories about what a summary covers  -*- lexical-binding: t; -*-

;;; Commentary:

;; A compaction is an entry whose only effect is on the projection: nothing is
;; deleted, and what it covers can be read back.  The threshold is a question,
;; not a policy, because a policy that fires silently is a policy nobody saw.

;;; Code:

(require 'support-tests)
(require 'pearl-compaction)
(require 'pearl-session)
(require 'pearl-contract)
(require 'ert-gwt)

;; A compaction is an entry, not a rewrite: the entries it covers stop being
;; projected and stay on the branch, which is what makes a summary reversible
;; and a replay honest (ADR 27).
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (compaction nil))
          (dolist (text '("one" "two" "three" "four"))
            (pearl-session-append
             session (pearl-contract-entry 'message (pearl-contract-message 'user text)))))
  (:when (setq compaction
               (pearl-compaction-append session "three rounds, summarised"
                                        :keep 1)))
  (:then (eq 'compaction (plist-get compaction :kind)))
  (:then (equal 3 (length (pearl-compaction-covers compaction))))
  (:then (equal 3 (length (pearl-compaction-covered session compaction))))
  (:then (equal 5 (length (pearl-session-entries session))))
  (:then (null (seq-find (lambda (message)
                           (equal "one" (plist-get message :content)))
                         (pearl-session-projection session nil))))
  (:then (seq-find (lambda (message)
                     (string-match-p "summarised"
                                     (format "%s" (plist-get message :content))))
                   (pearl-session-projection session nil))))

;; The threshold is a question the caller asks, and the answer is a name so a
;; second trigger can say its own.
(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (tight nil) (roomy nil) (estimate nil))
          (pearl-session-append
           session (pearl-contract-entry
                    'message (pearl-contract-message
                              'user (make-string 4000 ?x)))))
  (:when (progn
           (setq estimate (pearl-compaction-estimate session))
           (setq tight (pearl-compaction-needed session 100))
           (setq roomy (pearl-compaction-needed session 1000000))))
  (:then (>= estimate 1000))
  (:then (eq 'threshold tight))
  (:then (null roomy)))

;; A summary that fits comfortably does not cover anything: the cut is where
;; the budget ran out, and a budget that was never spent leaves the whole tail
;; in front of the reader.
(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (compaction nil) (folded nil))
          (dolist (text '("one" "two" "three"))
            (pearl-session-append
             session (pearl-contract-entry 'message (pearl-contract-message 'user text)))))
  (:when (progn
           (setq compaction (pearl-compaction-append session "nothing to fold"
                                                    :keep 10000))
           (setq folded (pearl-compaction-covers compaction))))
  (:then (null folded))
  (:then (equal 3
                (length (seq-filter
                         (lambda (message)
                           (not (eq 'system (plist-get message :role))))
                         (pearl-session-projection session nil))))))

;; A latch is what makes a fold cheap: an append changes nothing above it, so
;; the prefix a provider has already cached is the same prefix it is sent.
(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (before nil) (after nil))
          (pearl-session-append
           session (pearl-contract-entry 'message (pearl-contract-message 'user "one")))
          (pearl-session-append
           session (pearl-contract-entry 'message
                                         (pearl-contract-message 'assistant "two")))
          (setq before (pearl-session-projection session nil)))
  (:when (progn
           (pearl-session-append
            session (pearl-contract-entry 'message (pearl-contract-message 'user "three")))
           (setq after (pearl-session-projection session nil))))
  (:then (equal (1+ (length before)) (length after)))
  (:then (equal before (butlast after))))

;; The estimate counts a character without spaces as a token and four others as
;; one, because a page in Chinese is not a quarter of a page in English.
(ert-gwt-deftest
  (:given ((ascii (pearl-session-new)) (cjk (pearl-session-new))
           (ascii-tokens nil) (cjk-tokens nil) (difference nil))
          (pearl-session-append
           ascii (pearl-contract-entry 'message (pearl-contract-message
                                                 'user (make-string 400 ?x))))
          (pearl-session-append
           cjk (pearl-contract-entry 'message (pearl-contract-message
                                               'user (make-string 400 ?字)))))
  (:when (progn
           (setq ascii-tokens (pearl-compaction-estimate ascii))
           (setq cjk-tokens (pearl-compaction-estimate cjk))
           (setq difference (- cjk-tokens ascii-tokens))))
  (:then (>= difference 300))
  (:then (>= cjk-tokens 400)))

;; A cut never leaves a result behind: it belongs to the call that produced it,
;; and a context that keeps one without the other is not smaller, it is
;; malformed (ADR 27).
(ert-gwt-deftest
  (:given ((session (pearl-session-new)) (compaction nil) (next nil))
          (pearl-session-append
           session (pearl-contract-entry 'message (pearl-contract-message 'user "one")))
          (pearl-session-append
           session (pearl-contract-entry 'tool '(:name "read" :content "output")))
          (pearl-session-append
           session (pearl-contract-entry 'message
                                         (pearl-contract-message 'assistant "two"))))
  (:when (progn
           (setq compaction (pearl-compaction-append session "summarised" :keep 1))
           (setq next (seq-find (lambda (entry)
                                  (not (member (plist-get entry :id)
                                               (pearl-compaction-covers compaction))))
                                (pearl-session-entries session)))))
  (:then (>= (length (pearl-compaction-covers compaction)) 1))
  (:then (not (eq 'tool (plist-get next :kind)))))

;; Compacting twice leaves one summary in front of the model.  The second
;; entry's range covers the first entry, so the projection renders the newer
;; summary and not the one it replaced; the earlier entry stays on the branch,
;; because appending is the only thing this record does (ADR 27).
(ert-gwt-deftest
  (:given ((session (pearl-session-new))
           (first nil)
           (second nil)
           (projection nil))
          (dolist (text '("one" "two" "three" "four" "five" "six"))
            (pearl-session-append
             session (pearl-contract-entry 'message
                                           (pearl-contract-message 'user text))))
          (setq first (pearl-compaction-append session "the first summary" :keep 2))
          (pearl-session-append
           session (pearl-contract-entry 'message
                                         (pearl-contract-message 'user "seven"))))
  (:when (progn
           (setq second (pearl-compaction-append session "the second summary"
                                                 :keep 1))
           (setq projection (pearl-session-projection session nil))))
  (:then (member (plist-get first :id) (pearl-compaction-covers second)))
  (:then (seq-find (lambda (message)
                     (string-match-p "the second summary"
                                     (format "%s" (plist-get message :content))))
                   projection))
  (:then (null (seq-find (lambda (message)
                           (string-match-p "the first summary"
                                          (format "%s" (plist-get message :content))))
                         projection)))
  (:then (equal 9 (length (pearl-session-entries session)))))

(provide 'compaction-tests)

;;; compaction-tests.el ends here
