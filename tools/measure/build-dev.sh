#!/usr/bin/env bash
# Builds what the measuring tools run: the page with its development hooks (MELOET_DEV_HOOKS=1)
# and the release programs with theirs (--features dev-hooks): meloet, meloet-vkeys and
# meloet-plughost in target/release. A build without them is the one handed out
# (docs/architecture.md 23-11); the tools' MELOET_* settings do nothing there.
#   tools/measure/build-dev.sh            the page and the programs
#   tools/measure/build-dev.sh --keep-ui  only the programs; the app takes whatever ui/dist holds
set -euo pipefail

keep_ui=0
for arg in "$@"; do
  case "$arg" in
    --keep-ui) keep_ui=1 ;;
    *) echo "usage: $0 [--keep-ui]" >&2; exit 2 ;;
  esac
done

root=$(cd "$(dirname "$0")/../.." && pwd)
if [ "$keep_ui" = 0 ]; then (cd "$root/ui" && MELOET_DEV_HOOKS=1 npm run build); fi
(cd "$root" && cargo build --release -p meloet-app -p meloet-midi -p meloet-plughost --features dev-hooks)
