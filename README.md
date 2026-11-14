# meloet — foundation spikes

A DAW for people who just bought a MIDI keyboard. This repository does **not** hold the app yet.
It holds four throw-away experiments that check the four bets the design stands on.
The report is in [`docs/report.md`](docs/report.md).

| folder | bet | what it measures |
|---|---|---|
| `spikes/s1-render` | 1 · screen speed | Canvas 2D / WebGL piano roll with 10 000 notes, meters, in Tauri on WebKitGTK + NVIDIA |
| `spikes/plugins` | 2 · VST3, 3 · LV2 | scan in a child process, load, notes in → sound out, params, state, editor window, crash isolation |
| `spikes/s4-latency` | 4 · key response | hot-plug, key → Rust → lit key on screen, key → sound, buffer 256, Rust → UI transports |

License: MIT OR Apache-2.0. Every dependency is listed and checked by `licenses/check.mjs`.

## Setup (Ubuntu Studio 26.04)

```bash
sudo apt install libwebkit2gtk-4.1-dev libxdo-dev libayatana-appindicator3-dev librsvg2-dev \
  libasound2-dev libjack-jackd2-dev libpipewire-0.3-dev libspa-0.2-dev clang libclang-dev \
  liblilv-dev lv2-dev libsuil-dev libudev-dev mesa-utils lsp-plugins-vst3
# Rust (rustup) and Node.js LTS on PATH
(cd spikes/s1-render/ui && npm ci && npm run build)
(cd spikes/s4-latency/ui && npm ci && npm run build)
cargo build --release
```

## Running

```bash
# Bet 1: full matrix (env vars x size x renderer x meters + resize stress), then the table
spikes/s1-render/env.sh > spikes/s1-render/results/env.txt
spikes/s1-render/run_matrix.sh && python3 spikes/s1-render/summarize.py

# Bets 2 and 3
./target/release/pk-probe scan
./target/release/pk-probe test vst3 "/usr/lib/vst3/Surge XT.vst3#<class id from scan>"
./target/release/pk-probe test lv2 http://synthv1.sourceforge.net/lv2
./target/release/pk-probe crash vst3 "<id>" kill|segv|hang
spikes/plugins/run_all.sh          # every check against every target, results in spikes/plugins/results/

# Bet 4
spikes/s4-latency/run.sh           # see that script for the modes
```

## License check

```bash
node licenses/check.mjs            # exit 1 when a forbidden license appears
```
