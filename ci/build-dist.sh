#!/usr/bin/env bash
# `make dist`: build release artifacts into dist/. For a library workspace
# this packages each publishable crate; for a binary workspace it builds
# release binaries. Report what landed; never publish.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
. "$HERE/lib/util.sh"
ROOT="$(ci_repo_root)"
DIST="$ROOT/dist"
mkdir -p "$DIST"

# Release binaries, when the workspace builds any. A portable loop, not
# `find -perm`: BSD and GNU disagree on the syntax, and Windows binaries carry
# a .exe suffix. Copy regular executable files, skipping build byproducts.
cargo build --workspace --release --manifest-path "$ROOT/Cargo.toml"
for bin in "$ROOT"/target/release/*; do
    [ -f "$bin" ] && [ -x "$bin" ] || continue
    case "$bin" in
        *.d|*.so|*.dylib|*.dll|*.lib|*.exp|*.pdb|*.rlib) continue ;;
    esac
    cp "$bin" "$DIST/"
done

# Package crates that are not publish=false.
for manifest in "$ROOT"/crates/*/Cargo.toml "$ROOT"/Cargo.toml; do
    [ -f "$manifest" ] || continue
    if grep -q 'publish *= *false' "$manifest"; then
        continue
    fi
    dir="$(dirname "$manifest")"
    (cd "$dir" && cargo package --allow-dirty --no-verify --target-dir "$ROOT/target" \
        >/dev/null 2>&1) || true
done
find "$ROOT/target/package" -maxdepth 1 -name '*.crate' -exec cp {} "$DIST/" \; 2>/dev/null || true

printf 'dist artifacts:\n'
ls -1 "$DIST" 2>/dev/null | sed 's/^/  /' || printf '  (none)\n'