#!/usr/bin/env bash
# `make release-check`: the tag must agree with the version the workspace
# declares. A release tag vX.Y.Z is checked against the main crate's version
# (or the workspace package version). Fails when they disagree, so a tag is
# never published against a stale version.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib/util.sh"
ROOT="$(ci_repo_root)"
TOOLS="$(ci_tools_bin "$HERE")" || ci_die "could not build ci-tools"

TAG="${1:-$(git -C "$ROOT" describe --tags --exact-match 2>/dev/null || true)}"
[ -n "$TAG" ] || ci_die "no tag given and HEAD is not tagged"
VERSION="${TAG#v}"

MAIN="$("$TOOLS" config get --root "$ROOT" --suite "$HERE" main-crate 2>/dev/null || true)"
MANIFEST="$ROOT/Cargo.toml"
if [ -n "$MAIN" ] && [ -f "$ROOT/crates/$MAIN/Cargo.toml" ]; then
    MANIFEST="$ROOT/crates/$MAIN/Cargo.toml"
fi

DECLARED="$(python3 -c '
import sys, tomllib
with open(sys.argv[1], "rb") as fh:
    d = tomllib.load(fh)
v = d.get("package", {}).get("version") or d.get("workspace", {}).get("package", {}).get("version", "")
print(v)
' "$MANIFEST")"

if [ "$DECLARED" != "$VERSION" ]; then
    printf 'release-check: tag %s (version %s) does not match declared version %s in %s\n' \
        "$TAG" "$VERSION" "$DECLARED" "$MANIFEST" >&2
    exit 1
fi
printf 'release-check: %s matches declared version %s\n' "$TAG" "$DECLARED"