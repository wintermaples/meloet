#!/usr/bin/env bash
# Re-encodes the samples named in a list (from `sample-load list`) into one shipping format.
#
#   convert.sh <samples dir> <list.tsv> <out dir> <flac16|ogg6|ogg4> <full|trim> [jobs]
#
# The list has `name<TAB>frames`. With `trim`, each sample is cut at `frames` and its last 50 ms
# (2400 frames at 48 kHz) fade out. Needs ffmpeg and oggenc.
set -euo pipefail

if [ "${1:-}" = "--one" ]; then
    src="$2" out="$3" format="$4" cut="$5" name="$6" frames="$7"
    filter=()
    if [ "$cut" = trim ]; then
        start=$((frames > 2400 ? frames - 2400 : 0))
        filter=(-af "atrim=end_sample=${frames},afade=t=out:ss=${start}:ns=$((frames - start))")
    fi
    in="$src/$name.flac"
    case "$format" in
        flac16)
            ffmpeg -v error -nostdin -y -i "$in" "${filter[@]}" \
                -c:a flac -sample_fmt s16 -compression_level 8 "$out/$name.flac"
            ;;
        ogg6 | ogg4)
            ffmpeg -v error -nostdin -i "$in" "${filter[@]}" -c:a pcm_s24le -f wav - |
                oggenc -Q -q "${format#ogg}" -o "$out/$name.ogg" -
            ;;
        *)
            echo "unknown format $format" >&2
            exit 2
            ;;
    esac
    exit 0
fi

src="$1" list="$2" out="$3" format="$4" cut="$5" jobs="${6:-8}"
mkdir -p "$out"
tr '\t' '\n' <"$list" | xargs -d '\n' -n 2 -P "$jobs" "$0" --one "$src" "$out" "$format" "$cut"
