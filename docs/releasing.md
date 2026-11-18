# Releasing

What a release is, what it owes before the tag, and how the package reaches
MELPA. The promise in [README](../README.md) is only worth a listing when it
runs, so the recipe is submitted after the roadmap's stages are done — see
[roadmap.md](roadmap.md).

## What a tag is

- The `Version:` header in `pearl.el` is the version the tag names.
  `make release-check` asserts the two agree.
- `make dist` builds `dist/pearl-<version>.tar.gz` and `.zip` from `HEAD` with
  `git archive`, plus `SHA256SUMS`, for people who install by hand.
- `git tag -a v<version>` marks the commit. MELPA ignores tags and builds the
  default branch on every push, so a tag is for the archive, not for MELPA.

## Before the tag

1. `make lint`, `make test` and `make perf` are green. Lint is the same suite
   CI runs; a green local run is a green pipeline.
2. Re-read [README](../README.md), [architecture.md](architecture.md),
   [glossary.md](glossary.md), [roadmap.md](roadmap.md) and the records in
   [decisions/](decisions/). The tag is the one moment a stale sentence is
   published, and afterwards nobody knows which sentence was true when.
3. The README says what the tagged version can do: Installing for the install
   path, Using for the commands and keys, Configuration for the model, the
   credential and the limits. A section that does not exist yet is one this
   release has to write.
4. Run melpazoid, which builds the recipe and applies MELPA's checklist in
   MELPA's own Emacs:

   ```sh
   git clone https://github.com/riscy/melpazoid.git
   RECIPE='(pearl :fetcher github :repo "OverbearingPearl/pearl" :files ("pearl.el" "lisp/*.el"))' \
     python3 melpazoid/melpazoid.py
   ```

   It is not a gate because it costs a container that compiles Emacs from
   source; the recipe is the one thing it checks that `make lint` cannot see.
5. `make release-check`, then `make dist`, then the tag.

## Submitting to MELPA

- The recipe is [packaging/melpa/pearl](../packaging/melpa/pearl). It is
  copied verbatim into MELPA's `recipes/pearl` in a pull request against
  [melpa/melpa](https://github.com/melpa/melpa); the MELPA readme on recipes
  is the authority on the file spec if the layout changes.
- MELPA then builds the default branch from that recipe: it byte-compiles in
  the Emacs version its CI uses, runs `package-lint` and `checkdoc` under its
  own rules, checks the metadata, the license and that every file the recipe
  names can be loaded, and publishes a dated version on every push.
- MELPA does not run `make test`, does not run the suite in `ci/`, and does
  not exercise the promise. A recipe that builds is not a package that works;
  the stories are ours and stay ours.
- What the build rejects: a file that fails to byte-compile in a clean Emacs,
  a recipe naming a file that is not in the repository, a `Package-Requires`
  that names something MELPA cannot resolve, a main file with no autoload
  cookie, and a package whose headers disagree with its name.

## After

- A merged recipe means the package is published from the default branch
  within a day, with MELPA's date version; `Version:` in `pearl.el` stays the
  version `make dist` and `make release-check` speak about.
- Nothing in this repository changes when MELPA moves the recipe from
  `:fetcher github` to its own build: the file spec is the published one.
