# 20. Why the prompt is sections with one owner each

Date: 2026-11-18

## Status

Amended by [21. Why the prompt has one mechanism and not a seam beside it](0021-why-the-prompt-has-one-mechanism-and-not-a-seam-beside-it.md)

## Context

ADR 19 settled who writes the prompt: the package writes what is true of every
use, the repository writes what is true of this project. The mechanism it left
behind was one string and one replacing defvar, and putting a third author into
that shape exposed three problems that are not style.

The console composed the prompt by assigning `pearl-session-prompt-function`, a
defvar that belongs to the user and to extensions. Two consoles on two roots
would therefore overwrite each other's prompt, and the second one to be wired
would silently rewrite the first one's conversation. That is a bug the stories
did not catch, because every story so far has one console.

The seam was a whole-string replacement, so a user who wanted one extra
paragraph had to reproduce the shipped text, and with it the sentences that make
the product coherent. ADR 19 recorded that as the price of having a constant;
it is a price, but it was being paid for adding a paragraph, which is not the
same decision.

And three inputs had no single place that fixed their order. The projection had
`(or pearl-session-prompt-function (pearl-session-default-prompt …))`, the
console knew about files, and nothing anywhere said which came first. A prompt
whose order is decided at the call site is a prompt that reads differently
depending on who is asking.

## Options considered

- Keep one string and one defvar, and let the console write it. Cheap, and
  wrong the moment there are two consoles.
- Let `pearl` read the files and pass a finished string into the session. One
  owner, but the user's own file and the project's are then read at the entry
  point, and anything that constructs a session without going through `pearl`
  gets a prompt with no project in it.
- Sections with a fixed order, files read by the product, assembly done by the
  core, and a seam that appends.

## Decision

The prompt is a list of sections that share one shape, `(:name :path :text)`, in
an order the code fixes and nobody can reorder:

1. `preamble`, the shipped text from `pearl-session-default-prompt` over the
   loadout, untagged when it is rendered;
2. `user`, `pearl-user-instructions-file`, which defaults to
   `(locate-user-emacs-file "pearl/AGENTS.md")` and is nil to disable;
3. `project`, `AGENTS.md` at the console's root and no directory above it;
4. `extension`, the string `pearl-session-prompt-function` returns, appended
   last.

The core assembles and never reads: `pearl-session-prompt-sections` returns the
list, `pearl-session-prompt` renders it, and the session carries the sources the
product gave it through `pearl-session-set-prompt-sources`. Reading two files is
the console's job, because a root is the console's business, and a session that
opens a file is a core that cannot be tested with a plist.

The seam appends. `pearl-session-prompt-function` returns one more section and
cannot remove what came before; an extension that has to remove the shipped text
advises `pearl-session-prompt-sections`, which is what ADR 6 says extensions are
for. A user who wants different words instead of more words rebinds the
function, gets the addendum it produces, and carries the shipped text by
rebinding `pearl-session-default-prompt`'s caller through advice. The common
case stops paying the uncommon case's price.

## Consequences

Two consoles can hold two prompts, because the prompt is session data and not a
global. The order is code, so no configuration can put the project's habits
before the agent's own contract. A user's cross-project rules live in one file
in their Emacs directory rather than in every repository, and a project's
conventions still travel with the repository.

Cost: the session now carries something that is not an entry, which makes "a
session is a list of entries" slightly less true; the sources are data and not
state, so no command edits them. Reading is still at wire time, so an edit to
either file lands on the next console and not on the next turn. And because the
seam only appends, "I do not want the shipped sentences at all" is advice rather
than a variable, which is one more thing a user has to know how to write.
