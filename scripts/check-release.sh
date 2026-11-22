#!/usr/bin/env bash
# The build that is handed out carries none of the development hooks (docs/architecture.md 23-11).
#   scripts/check-release.sh            the page and the release programs without the hooks
#   scripts/check-release.sh --no-dev   only that (skips the build with the hooks, which shows
#                                       that the names looked for are the right ones)
# Builds ui/dist and target/release/{meloet,meloet-plughost}. The build with the hooks comes
# first, so that both are left as they are handed out.
set -euo pipefail

dev=1
for arg in "$@"; do
  case "$arg" in
    --no-dev) dev=0 ;;
    *) echo "usage: scripts/check-release.sh [--no-dev]" >&2; exit 2 ;;
  esac
done

cd "$(dirname "${BASH_SOURCE[0]}")/.."
root=$PWD
target=${CARGO_TARGET_DIR:-$root/target}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Every MELOET_ name is a hook; the other names are the hooks' own.
rust_names='MELOET_|devhooks|measure_enabled|measure_report|ui_script|key-to-sound\.json|vocal-faults\.log|urn:meloet:fake'
page_names='MELOET_|measure_enabled|measure_report|ui_script|uiScript|__meloetMeasure|key-to-screen|frame-times|pr-scroll'
# What the build with the hooks must show, one name from each place.
rust_must=(MELOET_PIPE MELOET_UI_SCRIPT MELOET_MEASURE_DIR MELOET_STATE_DIR MELOET_ACCEPT_VIRTUAL MELOET_PACK_ORIGIN MELOET_ENGINE_RT devhooks measure_report)
plughost_must=(MELOET_PLUGHOST_FAKE MELOET_PLUGHOST_RT urn:meloet:fake)
page_must=(measure_report ui_script pr-scroll)

failed=0
fail() { echo "check-release: $*" >&2; failed=1; }

build_page() {
  # $1: 1 with the hooks.
  (cd ui && if [ "$1" = 1 ]; then MELOET_DEV_HOOKS=1 npm run build; else env -u MELOET_DEV_HOOKS npm run build; fi) >"$work/page.log" 2>&1 ||
    { cat "$work/page.log" >&2; exit 1; }
}

build_programs() {
  # One package at a time: features of packages built together are joined.
  local features=()
  [ "$1" = 1 ] && features=(--features dev-hooks)
  cargo build --release -p meloet-app "${features[@]}"
  cargo build --release -p meloet-plughost "${features[@]}"
}

# The page inside the program, as it reads it (brotli undone).
embedded_page() {
  rm -rf "$work/embedded"
  mkdir -p "$work/embedded"
  MELOET_ASSETS_OUT="$work/embedded" cargo test --release -p meloet-app "$@" --quiet -- \
    --ignored --exact embedded_page::write_the_embedded_page >"$work/embedded.log" 2>&1 ||
    { cat "$work/embedded.log" >&2; exit 1; }
}

has_all() {
  # $1: what; $2: file of text; the rest: names.
  local what=$1 text=$2; shift 2
  for name in "$@"; do
    grep -qF -- "$name" "$text" || fail "$what has no $name: the names looked for are out of date"
  done
}

if [ "$dev" = 1 ]; then
  echo "== with the hooks (the names must be there)"
  build_page 1
  cat ui/dist/assets/*.js >"$work/page.txt"
  has_all "the page built with MELOET_DEV_HOOKS=1" "$work/page.txt" "${page_must[@]}"
  build_programs 1
  strings -n 6 "$target/release/meloet" >"$work/meloet.txt"
  strings -n 6 "$target/release/meloet-plughost" >"$work/plughost.txt"
  has_all "meloet built with dev-hooks" "$work/meloet.txt" "${rust_must[@]}"
  has_all "meloet-plughost built with dev-hooks" "$work/plughost.txt" "${plughost_must[@]}"
  embedded_page --features dev-hooks
  cat "$work"/embedded/assets/*.js >"$work/embedded.txt"
  has_all "the page inside meloet built with dev-hooks" "$work/embedded.txt" "${page_must[@]}"
  ls -l "$target/release/meloet" "$target/release/meloet-plughost"
fi

echo "== as handed out (no name may be there)"
build_page 0
if ls ui/dist/assets | grep -q '^start-'; then fail "ui/dist has the chunk of ui/src/dev (start-*.js)"; fi
if grep -rnoE "$page_names" ui/dist; then fail "ui/dist has the names above"; fi
build_programs 0
for program in meloet meloet-plughost; do
  if strings -n 6 "$target/release/$program" | grep -E "$rust_names" | sort | uniq -c; then
    fail "target/release/$program has the names above"
  fi
done
embedded_page
if ls "$work/embedded/assets" | grep -q '^start-'; then fail "the page inside meloet has the chunk of ui/src/dev"; fi
if grep -rnoE "$page_names" "$work/embedded"; then fail "the page inside meloet has the names above"; fi
ls -l "$target/release/meloet" "$target/release/meloet-plughost"

if [ "$failed" = 1 ]; then exit 1; fi
echo "== the release carries no development hook"
