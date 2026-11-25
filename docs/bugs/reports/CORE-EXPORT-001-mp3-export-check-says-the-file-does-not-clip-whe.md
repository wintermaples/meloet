---
id: CORE-EXPORT-001
title: mp3 export check says the file does not clip when 32-bit was chosen for wav before
area: CORE-EXPORT
kind: bug
severity: medium
status: reported
found: 2026-11-25
---

# CORE-EXPORT-001: mp3 export check says the file does not clip when 32-bit was chosen for wav before

## Where

crates/meloet-core/src/export.rs:919 (`check`: `let clips = over > 0 && self.bits != RecordFormat::Bits32 && !matched;`)
crates/meloet-core/src/export.rs:524-531 (`ExportChange::Format`: the bits are kept when the new format has no bits)
crates/meloet-core/src/export/sink.rs:384-391 (`Mp3Sink::push` clamps every sample to -1.0..=1.0)

## What happens

The check of the export dialog decides `clips` from `self.bits` alone. When the person chose 「32 ビット」 for wav in 「くわしく」 and then switches the format to mp3, `self.bits` stays `Bits32` (mp3 has no bits; they are kept for the other formats). So for an mp3 whose render goes over 0 dB, the check says `clips: false`: the screen shows only 「いちばん大きい所 … ・ 音割れなし」-style text with no clipping lines, and the button is 「書き出す」 instead of 「このまま書き出す」. But the mp3 sink clamps every sample to full scale, so the written mp3 is clipped at every place counted in `over`. The person is told the file is fine and gets a clipped file.

## What should happen

`ExportCheck::Done::clips` is "the file as it would be written with `bits` clips" (protocol `crates/meloet-protocol/src/export.rs`, doc of `ExportCheck`, spec 6 「書き出しで音が割れる」). An mp3 is always written within full scale (sink.rs, the comment of `Mp3Sink::push`: "the file holds what a player can play"), so for mp3 `clips` must be `over > 0` (unless matched), whatever bits were kept for wav. Only a 32-bit float wav keeps samples over 0 dB.

## How to trigger

Read from the code, not run.
1. Open 「書き出し…」 on a song whose master goes over 0 dB (for example one track with its fader at +6 dB).
2. Format wav, open 「くわしく」, choose 32 ビット.
3. Switch the format to mp3. Wait for the check to finish.
4. The check shows no clipping and the button says 「書き出す」; the mp3 written is clipped.
(The unit test `a_matched_file_does_not_clip_and_its_gain_can_be_down` only covers wav.)

## Cause

`check` looks at `self.bits` without looking at `self.format`. `ExportChange::Format` keeps `self.bits` when `format.bits_choices()` is empty (mp3, MIDI), as spec 3 F2 asks ("The bits chosen for wav and flac stay as they were"), so `Bits32` can be the kept value while the format is mp3.

## Suggested fix

In `check`, count the bits as floats only when the format takes them: for example
`let floats = self.format == ExportFormat::Wav && self.bits == RecordFormat::Bits32;`
`let clips = over > 0 && !floats && !matched;`
(flac cannot hold Bits32: `ExportChange::Format` resets it.) No protocol, wording or looks change. A test: wav 32 bits, then mp3, a render with `over > 0` gives `clips: true`.

## Verification

<!-- Filled by the verifier only: confirmed or not, how it was checked, corrections to the above. -->

## Fix

<!-- Filled after fixing: the commit, the test that now watches it. -->
