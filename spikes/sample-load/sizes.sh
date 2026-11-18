#!/usr/bin/env bash
# Total file size (MiB) of every shipping format, per plan.
#
#   sizes.sh <original samples dir> <work dir>
set -euo pipefail

orig="$1" work="$2"

total() {
    (cd "$1" && cut -f1 "$3" | sed "s/\$/.$2/" | xargs -d '\n' stat -c %s) |
        awk '{s += $1} END {printf "%.1f", s / 1048576}'
}

lists=(plan4-main plan4 plan6-main plan6)
printf 'format\t4 layers\t4 layers + extras\t6 layers\t6 layers + extras\n'
for d in orig24 flac16-full flac16-t60 flac16-t40 ogg6-full ogg6-t60 ogg6-t40 ogg4-full ogg4-t60 ogg4-t40; do
    dir="$work/$d" ext=flac
    case "$d" in
        orig24) dir="$orig" ;;
        ogg*) ext=ogg ;;
    esac
    printf '%s' "$d"
    for l in "${lists[@]}"; do
        printf '\t%s' "$(total "$dir" "$ext" "$work/lists/$l.tsv")"
    done
    printf '\n'
done
