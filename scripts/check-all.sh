#!/usr/bin/env bash
# Every check in one command. Stops at the first failure with a non-zero exit code.
#   scripts/check-all.sh            formatting, lints, tests, the UI checks, licenses
#   scripts/check-all.sh --devices  also the tests that need real devices (#[ignore])
set -euo pipefail

devices=0
for arg in "$@"; do
  case "$arg" in
    --devices) devices=1 ;;
    *) echo "usage: scripts/check-all.sh [--devices]" >&2; exit 2 ;;
  esac
done

cd "$(dirname "${BASH_SOURCE[0]}")/.."

step() {
  printf '\n== %s\n' "$*"
  "$@"
}

# The protocol tests write ui/src/bridge/generated and ui/tests/fixtures; the UI reads them,
# and meloet-app embeds ui/dist at compile time. So: protocol first, then the UI, then all of Rust.
step cargo test -p meloet-protocol
(
  cd ui
  step npm run check
  step npm test
  step npm run build
)
# Only the product crates; spikes/ is kept as it was written.
product=(); for d in crates/*/; do product+=(-p "$(basename "$d")"); done
step cargo fmt --check "${product[@]}"
step cargo clippy --all-targets -- -D warnings
step cargo test
if [ "$devices" = 1 ]; then
  step cargo test -- --ignored
fi
step node licenses/check.mjs

printf '\n== all checks passed\n'
