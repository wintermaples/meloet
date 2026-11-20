#!/bin/sh
# Makes the little audio files the tests of meloet-core read (tests/audio_files.rs,
# src/audiofiles/tests.rs). They are in the repository; run this only to make them again.
#
#   sh crates/meloet-core/tests/fixtures/make.sh
#
# Needs python3, ffmpeg, lame and oggenc. The sound is made here (a 441 Hz sine with one
# click in the middle), so nothing in the files is anybody else's.
set -eu
cd "$(dirname "$0")"

# click.wav: 1 second, 44100 Hz, 16 bits, stereo. Left: a 441 Hz sine at 0.25 with a click
# (one sample at 0.9) at frame 22050. Right: the same sine at half the size, no click.
python3 -I - <<'EOF'
import math, struct, wave
frames = 44100
with wave.open("click.wav", "wb") as w:
    w.setnchannels(2)
    w.setsampwidth(2)
    w.setframerate(44100)
    out = bytearray()
    for i in range(frames):
        s = 0.25 * math.sin(2 * math.pi * 441 * i / 44100)
        left = 0.9 if i == 22050 else s
        out += struct.pack("<hh", round(left * 32767), round(s * 0.5 * 32767))
    w.writeframes(bytes(out))
EOF

# The same audio in the other three kinds meloet reads. -bitexact keeps ffmpeg from writing
# its version into the files, so that they come out the same on another day.
ffmpeg -loglevel error -y -i click.wav -bitexact -c:a flac click.flac
lame --quiet -b 128 --resample 44.1 click.wav click.mp3
oggenc --quiet -q 2 -o click.ogg click.wav

# An mp3 without the tag that says how much silence the encoder put in front (lame -t).
lame --quiet -t -b 128 --resample 44.1 click.wav click-notag.mp3
# An mp3 as ffmpeg writes it (libmp3lame, with its own tag).
ffmpeg -loglevel error -y -i click.wav -bitexact -c:a libmp3lame -b:a 128k click-ffmpeg.mp3
# The tests write click.wav themselves (`write_click_wav`), with the same numbers.
rm click.wav
ls -l
