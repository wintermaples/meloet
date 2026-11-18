#!/usr/bin/env bash
# Fetch the sound pack named in tools/sounds/pack.json from its GitHub Release, check its sha256,
# and unpack it into ~/.cache/meloet/sounds/<version>/ (docs/architecture.md 19-1).
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$here/tools/sounds/pack.json"

field() {
  python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1], encoding="utf-8")).get(sys.argv[2], ""))' "$manifest" "$1"
}

url="$(field url)"
sha="$(field sha256)"
version="$(field version)"

if [ -z "$url" ]; then
  echo "音の包みは、まだ Release に上がっていません。tools/sounds/build.py で作ってください:" >&2
  echo "  python3 -I tools/sounds/build.py --out ~/.cache/meloet/sounds/<版>" >&2
  exit 1
fi
if [ -z "$sha" ] || [ -z "$version" ]; then
  echo "tools/sounds/pack.json に sha256 か version がありません。" >&2
  exit 1
fi
case "$version" in
  */* | .* | "") echo "版の名前がおかしい: $version" >&2; exit 1 ;;
esac

cache="${XDG_CACHE_HOME:-$HOME/.cache}/meloet/sounds"
dest="$cache/$version"
if [ -f "$dest/pack.json" ]; then
  echo "もうあります: $dest"
  exit 0
fi
mkdir -p "$cache"
work="$(mktemp -d "$cache/.fetch-XXXXXX")"
trap 'rm -rf "$work"' EXIT

echo "取ってきます: $url"
curl -fL --retry 3 -o "$work/pack.tar" "$url"

got="$(sha256sum "$work/pack.tar" | cut -d' ' -f1)"
if [ "$got" != "$sha" ]; then
  echo "sha256 が合いません。" >&2
  echo "  期待: $sha" >&2
  echo "  実際: $got" >&2
  exit 1
fi

top="meloet-sounds-$version"
# Only plain files and folders under meloet-sounds-<version>/ are accepted.
while IFS= read -r line; do
  type="${line:0:1}"
  if [ "$type" != "-" ] && [ "$type" != "d" ]; then
    echo "包みの中に、ふつうのファイルでない物があります: $line" >&2
    exit 1
  fi
done < <(tar -tvf "$work/pack.tar")
while IFS= read -r name; do
  case "$name" in
    "$top" | "$top/" | "$top"/*) ;;
    *) echo "包みの中に、外へ出る道があります: $name" >&2; exit 1 ;;
  esac
  case "/$name/" in
    */../*) echo "包みの中に、外へ出る道があります: $name" >&2; exit 1 ;;
  esac
done < <(tar -tf "$work/pack.tar")

mkdir "$work/out"
tar -xf "$work/pack.tar" -C "$work/out" --no-same-owner --no-same-permissions
mv "$work/out/$top" "$dest"
echo "置きました: $dest"
