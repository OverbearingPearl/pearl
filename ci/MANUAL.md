# The ci-rust manual

What each check wants, and how to satisfy it. The table of checks is generated
(`ci/run-checks.sh --list`), not committed; this file is the prose the table
cannot carry. A check named here behaves the same in every project that pulls
the suite — the project changes the *policy* in `config/ci.toml`, not the
check.

## How a run works

`run-checks.sh` discovers `checks/*.sh` sorted by filename. Each check answers
`--meta` with `name<TAB>scope<TAB>classes<TAB>description`. A `file`-scoped
check runs over the files whose class is in its class list; a `global` check
runs once over the repository. Every check emits TSV records on stdout:

```
SEVERITY<TAB>check<TAB>file<TAB>line<TAB>message
```

`SEVERITY` is `ERROR` or `WARN`. The orchestrator prints them as
`file:line: [check] message`, adds GitHub annotations under Actions, and exits
nonzero iff there is at least one `ERROR`. A check always exits 0 itself; the
gate is the orchestrator's, counted from the records. This is why a check can
report a hundred problems and still let the next check run.

## Classes

A file's class is decided by where it lives, not by what it says:

- `main` — the root crate's entry source, when `main-crate` names it.
- `business` — a `.rs` under `src-dir` (default `crates`) or `src/`.
- `test` — under `test-dir` (default `tests`), a `tests/` directory, or a
  `*-tests.rs` / `*_tests.rs` file.
- `tooling` — under `tooling-dir` (default `ci`) or `tools/`.
- `vendor` — under `vendor-dir` (default `vendor`).

A file in none of these is reported by `--files` as outside the package and
is not checked. `vendor` is exempt from every style rule; the one rule that
reaches it is `dependency-direction` (a vendored crate may depend only on
another vendored crate).

## The tool checks

### 00-rustdoc
`cargo doc --workspace --no-deps` with `RUSTDOCFLAGS="-D warnings"`. A missing
doc comment on a public item, a broken intra-doc link, or any rustdoc warning
is an ERROR. Satisfy it by documenting public items and fixing link targets.

### 01-cargo-lint
`cargo clippy --workspace --all-targets -- -D warnings`. Every clippy lint is
an ERROR at its primary span. Satisfy it by fixing the lint or, where a lint is
wrong for the code, `#[allow(...)]` with a reason. Do not blanket-allow.

### 02-build
`cargo build --workspace`. The workspace must compile. A hard failure with no
diagnostic becomes one ERROR record so the gate still trips.

## The structural checks

### 03-blank-lines
No file opens with a blank line; no three consecutive blank lines anywhere.
One blank line separates items; two is the most a gap ever needs.

### 04-header-comment
A `business` or `main` module opens with a `//!` module doc, not a plain `//`
comment and not bare code. `test` and `tooling` files may open with a plain
comment. `vendor` is exempt.

### 05-comment-width
Comment lines stay within `comment-width` columns (default 80). A line carrying
a URL is excused. Code lines are not this check's business — `rustfmt` owns
those.

### 06-banned-construct
No line matches a `[[banned]]` regex from the policy. Each match shows the
configured suggestion. Empty by default; a project adds its own (for example
`dbg!`, or `unwrap` in business code). `vendor` is exempt.

### 07-meta-info
Every `[package]` manifest carries `name`, `version`, `license`, `description`
and `edition`. A pure workspace manifest (no `[package]`) owes nothing.

### 08-module-name
A `.rs` file's stem is lower snake_case (`[a-z0-9_-]+`). `mod.rs`, `lib.rs` and
`main.rs` pass. A CamelCase or spaced file name is an ERROR.

### 09-crate-prefix
When `crate-prefix` is set, every `business`/`main` crate's name starts with
it. This keeps a workspace's crates namespaced. Empty prefix disables it.

### 10-naming
Types (`struct`/`enum`/`trait`/`type`) are CamelCase; functions are snake_case.
A cheap regex pass — clippy catches the rest. `vendor` is exempt.

### 11-test-suffix
A file named `*-tests.rs` / `*_tests.rs` must live in the `test` class. A
business module must not wear the test suffix.

### 12-dependency-direction
A crate may depend only on crates whose class is in its profile's
`may-require`. Read from `cargo metadata` (workspace members only; external
crates are ignored). `vendor` may depend only on `vendor`. This is the rule
that reaches vendored code.

### 13-msrv
A crate's `rust-version` is at or above the suite floor (`msrv`, default
1.85). A crate that declares an older floor fails rather than lowering the
suite's.

### 14-edition
A crate's `edition` equals the configured `edition` (default 2024). One edition
across the workspace.

### 15-text-hygiene
LF endings (no CRLF), exactly one final newline, no trailing whitespace. A tab
is a WARN (rustfmt uses spaces). `vendor` is exempt.

### 16-policy
The configuration is self-consistent: every class has a profile; every layer
in `order` has members; every crate named in a layer exists; every `[[banned]]`
regex compiles. A policy that contradicts itself is an ERROR before any code is
read.

### 17-project-neutral
The suite's own source (`ci/` and `tools/`) must not name the project it
happens to be checking — no occurrence of `crate-prefix` in suite `.rs`/`.sh`/
`.toml`. The suite is generic; a project name in it is a leak.

### 18-doc-links
Every relative Markdown link target exists, and every `#anchor` resolves to a
topic slug in the target file. External (`http`, `mailto`) links are not
checked. A page that moves takes its inbound links with it.

### 19-header-name
When a module opens with a `//! name --- description` header, `name` matches
the file stem. A header that names a different module is an ERROR. Modules
without that header shape are left alone.

### 20-adr-format
Each `docs/decisions/NNNN-*.md` (except `0000-*` and `superseded/`) keeps the
Nygard shape this suite expects: a `# N. Title` topic, a `Date:` line, and
`## Status`, `## Context`, `## Options considered`, `## Decision`,
`## Consequences` sections, the last ending with a line beginning `Cost:`. A
choice with no stated price is not a choice. No-op when `docs/decisions/` is
absent.

### 21-layer-order
When `[layers] order` is non-empty, every `business`/`main` crate must belong
to a layer (else "an undeclared crate is one nobody placed"), and may depend
only on crates in its own layer or a preceding one (or its `may-require`
override). Read from `cargo metadata`. No-op when no layers are declared.

### 22-stub-orphans
No `todo!()` or `unimplemented!()` left in `business`/`main` code. A stub is
invisible once the real module lands and the story passes against it; this
check reports the stub that should have been deleted. Test code may stub
freely.

## The reports (not checks)

`make size`, `make coverage` and `make perf` print and never fail. `make
perf-check` is the gated form, run nightly. `make release-check` fails when the
tag and the declared crate version disagree. These are gates and reports about
the whole workspace, not per-file findings, so they live outside `checks/`.

## Adding a check

Drop `checks/NN-name.sh` into the suite (a project that wants a private check
puts it in `config/targets.mk` wiring instead, since `ci/` is replaced on a
pull). Answer `--meta`, emit TSV records, exit 0. The table regenerates; there
is nothing to register by hand.