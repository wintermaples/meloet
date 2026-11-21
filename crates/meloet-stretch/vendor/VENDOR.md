# Vendored C++ (not changed)

The files here are copies, byte for byte. Do not edit them. `node licenses/check.mjs` compares
their sha256 with `licenses/upstream.json`.

## Signalsmith Stretch

- From: https://github.com/Signalsmith-Audio/signalsmith-stretch
- Tag `1.4.0`, commit `a670068d9aeb64913331d5cc29337b19a457a7df` (2026-09-25). The newest tag on 2026-11-21.
  `main` was one commit ahead (`88061fd`, 2026-10-22); that commit only changes `CMakeLists.txt`
  (Linear 0.6.4 to 0.6.6), no header.
- The header itself says `version[3] = {1, 3, 2}`; the tag is the version we name.
- License: MIT, `Copyright (c) 2022 Geraint Luff / Signalsmith Audio Ltd.` (`LICENSE.txt`). The header has no license line of its own.

| File here | File there | sha256 |
|---|---|---|
| `signalsmith-stretch/signalsmith-stretch.h` | `signalsmith-stretch.h` | `1188667959ac19dd40c0a6abbce694e44705615ec4f0e8db8af0f1cfb4c5dea7` |
| `signalsmith-stretch/LICENSE.txt` | `LICENSE.txt` | `ee2ef82481ffb445ecdd4b3a4c1f82c0cddb2da8fe39d8e8dc384fffb3e7f06f` |

## Signalsmith Linear

- From: https://github.com/Signalsmith-Audio/linear
- Tag `0.6.4`, commit `de55e6a50ffcf6f8f43f649692d94691c7025151` (2026-09-25). This is the tag that
  `CMakeLists.txt` of Signalsmith Stretch 1.4.0 asks for (`GIT_TAG 0.6.4`).
- License: MIT, `Copyright (c) 2025 Signalsmith Audio` (`LICENSE.txt`). The headers have no license line of their own.
- Only what `signalsmith-stretch.h` includes: `signalsmith-linear/stft.h` → `stft.h` → `fft.h`.
  `linear.h`, `approx.h` and `platform/` are not included without a `SIGNALSMITH_USE_*` define (we set none:
  the FFT is the built-in one).

| File here | File there | sha256 |
|---|---|---|
| `signalsmith-linear/stft.h` | `stft.h` | `7422ff5ddc2a017dc0587dcce99ce5ba30a2414bc2996081f6b00d56905d9b8c` |
| `signalsmith-linear/fft.h` | `fft.h` | `fb55f69ac14cdbac66b9c76053069ff63bc36d18af20d8c028b9acc0aad6516a` |
| `signalsmith-linear/include/signalsmith-linear/stft.h` | `include/signalsmith-linear/stft.h` | `a323656e5ed80b271262106ea782d6150ef2f1c8b2f3c5896552b90b2ad6c17e` |
| `signalsmith-linear/LICENSE.txt` | `LICENSE.txt` | `072b5e9eb5b22880bdf6256324654cf3ce53dba828edb911921121e1be09d9c3` |

## Taking them again

```bash
git clone https://github.com/Signalsmith-Audio/signalsmith-stretch ss
git -C ss checkout a670068d9aeb64913331d5cc29337b19a457a7df
git clone https://github.com/Signalsmith-Audio/linear lin
git -C lin checkout de55e6a50ffcf6f8f43f649692d94691c7025151   # the GIT_TAG in ss/CMakeLists.txt

V=crates/meloet-stretch/vendor
cp ss/signalsmith-stretch.h ss/LICENSE.txt $V/signalsmith-stretch/
cp lin/stft.h lin/fft.h lin/LICENSE.txt $V/signalsmith-linear/
cp lin/include/signalsmith-linear/stft.h $V/signalsmith-linear/include/signalsmith-linear/
(cd $V && find . -type f ! -name VENDOR.md | sort | xargs sha256sum)
```

For a newer version: read both `LICENSE.txt` and the top of every header again, follow the `#include`
lines again, then update this file, `licenses/upstream.json` (version, commit, sha256) and
`STRETCH_VERSION` in `src/lib.rs` (it is part of the key of made sounds).
