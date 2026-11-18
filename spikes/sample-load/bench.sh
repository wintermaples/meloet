#!/usr/bin/env bash
# Times `sample-load load` over the shipping formats. Every run is its own process.
#
#   bench.sh <sample-load binary> <original samples dir> <work dir> warm|cold [runs]
#
# warm: the files are in the page cache (one unmeasured run first).
# cold: before every run the kernel is asked to forget these files only (posix_fadvise DONTNEED,
#       no sudo, nothing system-wide), and `fincore` shows how much stayed cached.
# Output: one line per case, `min / median / max` of the wall time in ms.
set -euo pipefail

bin="$1" orig="$2" work="$3" mode="$4" runs="${5:-3}"

cases=(
    "$orig|flac|orig24"
    "$work/flac16-full|flac|flac16-full"
    "$work/flac16-t60|flac|flac16-t60"
    "$work/flac16-t40|flac|flac16-t40"
    "$work/ogg6-full|ogg|ogg6-full"
    "$work/ogg6-t60|ogg|ogg6-t60"
    "$work/ogg6-t40|ogg|ogg6-t40"
    "$work/ogg4-full|ogg|ogg4-full"
    "$work/ogg4-t60|ogg|ogg4-t60"
    "$work/ogg4-t40|ogg|ogg4-t40"
)
if [ "$mode" = cold ]; then
    formats=(i16)
    threads=(1 8)
else
    formats=(i16 f32)
    threads=(1 4 8 16)
fi

cached_mib() {
    # Sum of the RES column of fincore, in MiB.
    (cd "$1" && cut -f1 "$3" | sed "s/\$/.$2/" | xargs -d '\n' fincore --bytes --noheadings --output RES) |
        awk '{s += $1} END {printf "%.1f", s / 1048576}'
}

echo "plan	format	to	threads	files	file MiB	audio s	memory MiB	ms min / median / max	RSS MiB before -> after	cached MiB before run"
for plan in 4 6; do
    list="$work/lists/plan$plan.tsv"
    for c in "${cases[@]}"; do
        IFS='|' read -r dir ext label <<<"$c"
        for fmt in "${formats[@]}"; do
            for t in "${threads[@]}"; do
                cached=-
                if [ "$mode" = warm ]; then
                    "$bin" load "$dir" "$ext" "$list" "$fmt" "$t" >/dev/null
                fi
                lines=()
                for _ in $(seq "$runs"); do
                    if [ "$mode" = cold ]; then
                        "$bin" evict "$dir" "$ext" "$list"
                        cached=$(cached_mib "$dir" "$ext" "$list")
                    fi
                    lines+=("$("$bin" load "$dir" "$ext" "$list" "$fmt" "$t")")
                done
                printf '%s\n' "${lines[@]}" | sort -t '	' -k9,9n | awk -F '\t' \
                    -v plan="$plan" -v label="$label" -v cached="$cached" '
                    { ms[NR] = $9; row = $0 }
                    END {
                        split(row, f, "\t")
                        printf "%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%.0f / %.0f / %.0f\t%s -> %s\t%s\n",
                            plan, label, f[3], f[4], f[5], f[6], f[7], f[8],
                            ms[1], ms[int((NR + 1) / 2)], ms[NR], f[10], f[11], cached
                    }'
            done
        done
    done
done
