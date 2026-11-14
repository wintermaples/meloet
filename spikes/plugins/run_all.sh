#!/usr/bin/env bash
# Runs scan, the full test and the crash tests against every target. Results: spikes/plugins/results/*.json
set -u
cd "$(dirname "$0")/../.."
P=./target/release/pk-probe
OUT=spikes/plugins/results
mkdir -p "$OUT"
export DISPLAY=${DISPLAY:-:0} PIPEWIRE_REMOTE=${PIPEWIRE_REMOTE:-/run/user/1000/pipewire-0} PIPEWIRE_RUNTIME_DIR=${PIPEWIRE_RUNTIME_DIR:-/run/user/1000}

# A deliberately broken VST3 next to the real ones: its factory segfaults.
CRASHY=$OUT/crashy.vst3
mkdir -p "$CRASHY/Contents/x86_64-linux"
cc -shared -fPIC -o "$CRASHY/Contents/x86_64-linux/crashy.so" spikes/plugins/tests/crashy.c
# The scan takes ~15 min with 650 LV2 plugins; reuse it unless RESCAN=1.
if [ -n "${RESCAN:-}" ] || [ ! -s "$OUT/scan.json" ]; then
  PK_EXTRA_VST3=$CRASHY $P scan > "$OUT/scan.json" 2> "$OUT/scan.err"
fi

# Pick targets from the scan by name.
read -r -d '' PICK <<'EOF'
import json, sys
scan = json.load(open(sys.argv[1]))
want = {
  "vst3:Surge XT": ("vst3", "Surge XT"),
  "vst3:LSP Compressor Stereo": ("vst3", "Compressor Stereo"),
  "lv2:Surge XT": ("lv2", "Surge XT"),
  "lv2:synthv1": ("lv2", "synthv1"),
  "lv2:Dragonfly Hall Reverb": ("lv2", "Dragonfly Hall Reverb"),
  "lv2:LSP Compressor Stereo": ("lv2", "Compressor Stereo"),
}
for label, (fmt, name) in want.items():
    hits = [p["plugin"] for p in scan["plugins"] if p["plugin"]["format"].lower() == fmt and p["plugin"]["name"].endswith(name)]
    if not hits:
        print(f"{label}\t-\tNOT FOUND"); continue
    h = hits[0]
    pid = f'{h["path"]}#{h["id"]}' if fmt == "vst3" else h["id"]
    print(f"{label}\t{fmt}\t{pid}")
EOF
python3 -I -c "$PICK" "$OUT/scan.json" > "$OUT/targets.tsv"
cat "$OUT/targets.tsv"

while IFS=$'\t' read -r label fmt id; do
  [ "$fmt" = "-" ] && continue
  slug=$(echo "$label" | tr ' :' '__')
  echo "== $label"
  timeout 120 $P test "$fmt" "$id" ${PLAY:+--play} > "$OUT/test__$slug.json" 2> "$OUT/test__$slug.err"
  echo "exit=$?"
done < "$OUT/targets.tsv"

# Crash isolation, research level: one instrument per format.
for pair in "vst3:Surge XT" "lv2:synthv1"; do
  line=$(grep -P "^$pair\t" "$OUT/targets.tsv") || continue
  fmt=$(echo "$line" | cut -f2); id=$(echo "$line" | cut -f3)
  for mode in kill segv hang; do
    slug=$(echo "$pair" | tr ' :' '__')
    timeout 60 $P crash "$fmt" "$id" $mode > "$OUT/crash__${slug}__$mode.json" 2>/dev/null
    echo "crash $pair $mode exit=$?"
  done
done
