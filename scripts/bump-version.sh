#!/usr/bin/env bash
# Raise the app version everywhere it is written, then check that the old one is gone.
#   scripts/bump-version.sh 0.2.0
# It changes: every crates/*/Cargo.toml, Cargo.lock (workspace members only), NOTICE and the
# about dialog list (tools/licenses/inventory.mjs write), and the deb name and version in the docs.
# It does not commit, tag, push or build the deb.
set -euo pipefail

new="${1:-}"
if [[ ! "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "usage: scripts/bump-version.sh X.Y.Z" >&2
  exit 2
fi

cd "$(dirname "${BASH_SOURCE[0]}")/.."

old="$(sed -n 's/^version = "\([0-9.]*\)"$/\1/p' crates/meloet-app/Cargo.toml | head -1)"
if [[ -z "$old" ]]; then
  echo "cannot read the version of crates/meloet-app/Cargo.toml" >&2
  exit 1
fi
if [[ "$old" == "$new" ]]; then
  echo "the version is already $new" >&2
  exit 1
fi
echo "== $old -> $new"

# 1. The product crates all carry the same version.
for toml in crates/*/Cargo.toml; do
  if ! grep -q "^version = \"$old\"$" "$toml"; then
    echo "$toml does not have version $old; fix it by hand first" >&2
    exit 1
  fi
done
for toml in crates/*/Cargo.toml; do
  sed -i "0,/^version = \"$old\"$/s//version = \"$new\"/" "$toml"
done

# 2. Cargo.lock: only the workspace members change.
cargo update --workspace --offline

# 3. NOTICE and ui/src/components/about/generated/licenses.json carry the app version.
node tools/licenses/inventory.mjs write

# 4. The docs that name the deb or the current version.
esc_old="${old//./\\.}"
sed -i "s/meloet_${esc_old}_amd64\.deb/meloet_${new}_amd64.deb/g" README.md README.ja.md docs/development/ubuntu.md docs/install-check.md docs/user/README.ja.md docs/progress.md
sed -i "s/いまの版は ${esc_old} です/いまの版は ${new} です/" docs/user/README.ja.md
sed -i "s/^| App | ${esc_old} /| App | ${new} /" docs/progress.md

# 5. Nothing that names the current version may still say the old one.
left=0
check() {
  local file="$1" pattern="$2"
  if grep -n -- "$pattern" "$file"; then
    echo "  ^ still $old in $file" >&2
    left=1
  fi
}
for toml in crates/*/Cargo.toml; do check "$toml" "^version = \"$old\"$"; done
check NOTICE "^Version $old\."
check ui/src/components/about/generated/licenses.json "\"meloet\": {\"version\":\"$old\""
for doc in README.md README.ja.md docs/development/ubuntu.md docs/install-check.md docs/user/README.ja.md docs/progress.md; do
  check "$doc" "meloet_${old}_amd64"
done
check docs/user/README.ja.md "いまの版は $old"
check docs/progress.md "^| App | $old "
if grep -A1 -E '^name = "meloet(-[a-z]+)?"$' Cargo.lock | grep -q "^version = \"$old\"$"; then
  echo "  ^ Cargo.lock still has a workspace member at $old" >&2
  left=1
fi
if [[ "$left" != 0 ]]; then
  echo "== some places still say $old (above)" >&2
  exit 1
fi

echo "== every place says $new. Review with: git diff --stat"
echo "   Old records in docs/history/ and the first-version note in docs/architecture.md are left as they were."
