# Key latency — release, window 1920x1080, 120 keys

- buffer: 256 frames running (asked for 256), 48000 Hz
- keys sent by meloet-vkeys: 120

## Key -> screen (ms)

| | Rust -> JS p50 / p95 / max | Rust -> drawn (rAF) p50 / p95 / max | n |
|---|---|---|---|
| this run | 2.78 / 11.82 / 15.80 | 3.79 / 13.80 / 16.80 | 120 |
| study (real keys) | 0.02 / 0.41 / — | 0.28 / 0.84 / 5.20 | 117 |

## Key -> sound (ms)

| | Rust -> callback mean / p95 / max | Rust -> DAC (planned) mean / p95 / max | n |
|---|---|---|---|
| this run | 2.56 / 4.91 / 4.93 | 13.22 / 15.58 / 15.60 | 120 |
| study (real keys, quantum 256) | 2.80 / 5.20 / — | 13.50 / 15.80 / 16.00 | — |

- shown within 2 frames (drawn + one 60 Hz frame, an estimate): 98 %
- meloet-vkeys sent -> Rust received, p50 / p95 / max: 0.03 / 0.11 / 0.22 ms
