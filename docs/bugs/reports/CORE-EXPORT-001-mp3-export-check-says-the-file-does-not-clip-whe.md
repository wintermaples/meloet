---
id: CORE-EXPORT-001
title: mp3 export check says the file does not clip when 32-bit was chosen for wav before
area: CORE-EXPORT
kind: bug
severity: medium
status: fixed
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

Confirmed by reading (no test run). `check` (`export.rs:919`): `clips = over > 0 && self.bits != RecordFormat::Bits32 && !matched`, with no look at `self.format`. `ExportChange::Format` (`export.rs:524-531`) keeps `self.bits` when the new format has no bits choices (mp3, MIDI), so `Bits32` chosen for wav stays while the format is mp3. `Mp3Sink::push` (`export/sink.rs:380-391`) clamps every sample to ±1.0. So the check says "no clipping" for an mp3 that is written clipped. Severity `medium` stands (the person is told the file is fine; the way around is to set wav back to 24 bits).

Corrections: none.

Fix plan: `crates/meloet-core/src/export.rs` `check`: `let floats = self.format == ExportFormat::Wav && self.bits == RecordFormat::Bits32; let clips = over > 0 && !floats && !matched;` (ask `self.format.bits_choices().contains(&RecordFormat::Bits32)` instead of naming wav if that reads better). Look for other readers of `self.bits` that forget the format (the done text, the job's own bits) while there. Test in `export/tests.rs` or `mp3_tests.rs`: wav 32 bits, then mp3, a render with `over > 0` → `clips: true`.

## Fix

`crates/meloet-core/src/export.rs` `check`: the bits count as floats only when the format takes 32 bits (`self.bits == Bits32 && self.format.bits_choices().contains(&Bits32)`), so an mp3 with over > 0 clips whatever bits were kept for wav. The other readers of `self.bits` were looked at: the job's target and the done view carry the bits, but the mp3 sink does not read them and the done view shows the bitrate for mp3. `docs/architecture.md` 23-7 and 23-8 say "a 32-bit wav".

Test: `export::tests::an_mp3_clips_whatever_bits_were_kept_for_wav` (`export/tests.rs`).
