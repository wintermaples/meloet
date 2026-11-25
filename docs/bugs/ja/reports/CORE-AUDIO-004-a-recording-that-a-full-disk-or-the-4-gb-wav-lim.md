---
id: CORE-AUDIO-004
title: ディスクがいっぱいになる、または wav の 4 GB 上限で止まる録音は、音の装置が止めた録音と違い、何も知らせない
area: CORE-AUDIO
kind: ui-ux
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-AUDIO-004: ディスクがいっぱいになる、または wav の 4 GB 上限で止まる録音は、音の装置が止めた録音と違い、何も知らせない

## Where

crates/meloet-core/src/vocalrec.rs:959-962 (`Job::write`: 余地のないファイルは `said.full` を立てる)
crates/meloet-core/src/vocalrec.rs:684-686, 988-990 (書き込みか `fdatasync` が失敗すると `said.error` を立てる)
crates/meloet-core/src/service/input.rs:423-427 (`poll_input`: `full` か `error` なら `Input::VocalsCannotGoOn`)
crates/meloet-core/src/state.rs:1409-1413 (`VocalsCannotGoOn` は `stop_recording` を呼ぶだけで、知らせを立てない)

## What happens

ディスクがいっぱいになる、声のファイルの書き込みか同期が失敗する、またはファイルが wav に入る最大の大きさに達すると (48 kHz、24 bit、ステレオで約 4 時間 8 分。96 kHz、32 bit、ステレオで 93 分)、ディスクに書く処理は状態を印し、コアは使う人が止めたのとまったく同じように録音を止めます。テイクは残りますが、状態行には止まった理由が何も出ません。標準エラーには「録音は全部は書けませんでした」と出る (エラーの場合のみ。4 GB の上限では何も出ない) だけです。使う人には、録音が勝手に終わったように見え、同じ満杯のディスクへもう一度録音を始めるかもしれません。

## What should happen

docs/architecture.md 22-7: 「4 GB limit …: stop recording before the limit and keep what was recorded (same path as 「the sound stopped」, 4-2)」(4 GB の上限: 上限の前に録音を止め、録れた分を残す。4-2 の「音が止まった」と同じ道)。4-2: 「If the sound stops while recording, the recording is closed …, what was recorded is kept as a take, and a notice is shown」(録音中に音が止まったら、録音は閉じられ、録れた分はテイクとして残り、知らせを出す)。対になる、流れが切れた場合は知らせを出しています: `cut_notice` (state/vocal.rs:65-73、`RecordingStoppedAudioSystem` / `RecordingStoppedOutputMissing`、仕様 6 の「…録音を止めました。そこまでの演奏は テイク 2 に残っています。」)。

## How to trigger

コードを読んで確かめた内容で、実行はしていません。

1. 状態のフォルダを、ほぼいっぱいのファイルシステムに置く (または録音中にディスクをいっぱいにする)。
2. 書き込みが「no space」で失敗するまで、歌のトラックを録音する。
3. 録音が止まる。テイクは残っているが、理由は何も出ない。

(4 GB の場合: 96 kHz、32 bit のステレオで歌のトラックを 93 分録音する。)

## Cause

`Input::VocalsCannotGoOn` は理由もテイクも持たず、state.rs のその腕は `StatusMessage` を立てません。`vocals_written` は、流れが切れた場合にだけ `Cut` を返します。

## Suggested fix

理由 (`full` か error) を録音と一緒に保つ。流れが切れた場合に `Cut` を保つのと同じ。テイクが入ったら、`cut_notice` のように知らせを出す。「ディスクがいっぱい」と「ファイルが最大の大きさに達した」のための文面が要る (仕様 6 にはどちらもない)。だから、リードかオーナーが文面を決める。新しい `StatusMessage` の腕はプロトコルの追加になる。

## Verification

実際の問題。コードを読んで確かめた (テストは実行していない)。`Job::write` はファイルに余地がなくなると `said.full` を立て (`vocalrec.rs:959-962`)、書き込みか同期が失敗すると `said.error` を立てる。`poll_input` (`service/input.rs:423-427`) はどちらも `Input::VocalsCannotGoOn` に変える。その腕 (`state.rs:1409-1413`) は `stop_recording` を呼ぶだけ。テイクは残るが、知らせは立たない。流れが切れた場合は `cut_notice` (`RecordingStoppedAudioSystem` / `RecordingStoppedOutputMissing`) で知らせが出る。architecture 22-7 は 4 GB での停止を「音が止まったのと同じ道 (4-2)」とし、4-2 は「知らせを出す」と書いている。

既存の文面を探した: `RecordingStoppedAudioSystem` は PipeWire の名前を出す。`RecordingStoppedOutputMissing` は出口の名前を出す。`InputLost` は抜かれた入口の名前を出す (「…録音を止めました。録れた所までは残しています。」)。`RecordingGaps` と `DiskTooSlow` は飛びについての知らせ。どれも、違う原因を言ってしまうので使えない。`ja.ts` には `pack.diskFull` (音の素材について) はあるが、録音についてのものはない。

`needs-lead` にする理由: 二つの原因 (ディスクに書けない、ファイルが wav に入る最大の大きさに達した) の新しい文面と、新しい `StatusMessage` の腕 (プロトコルの追加)。選択肢:
1. 理由 (`full` / `error`) つきの新しい腕を一つ作り、`cut_notice` と同じく、テイクが入ったら言う。例: 「ディスクに書けなくなったので、録音を止めました。録れた所までは残しています。」と「ファイルがいちばん大きい所まで来たので、録音を止めました。録れた所までは残しています。」(後半は `msg.inputLost` の文面)。理由は `Input::VocalsCannotGoOn { full }` から録音に付いて行く。推奨。文面はオーナーが決める。
2. 二つの原因に一つの文面 (短いが、正確さは劣る)。
3. そのままにして、`docs/progress.md` に書く。

重大度 `medium` は変えない (テイクは残るが、録音が終わった理由を使う人に知らせず、同じ満杯のディスクへもう一度録音できてしまう)。

## Fix

プロトコル 38 (リードの決定): `StatusMessage::RecordingStoppedDisk { reason }` と `DiskStop::{CannotWrite, FileFull}` (`crates/meloet-protocol/src/lib.rs`、`vocal.rs`)。`Input::VocalsCannotGoOn` は理由を持つ (`crates/meloet-core/src/service/input.rs` の `poll_input`: 書き込みの失敗は `CannotWrite`、それ以外は wav の大きさの上限 `FileFull`)。その腕 (`crates/meloet-core/src/state.rs`) は理由を `Cut::Disk` として録音と一緒に保つ (`crates/meloet-core/src/play.rs`、`play/vocal.rs` の `vocals_cut`)。`cut_notice` (`state/vocal.rs`) は、テイクが入ったところで一度だけ言う。流れが切れた場合と同じ。文面は `msg.recordingStoppedDisk` と `msg.recordingStoppedFileFull` (`builder:2026-11-25`)、`StatusLine.svelte`。テスト: `state::vocal_tests::a_recording_its_files_cannot_be_written_on_stops_and_says_why_once_the_take_is_in`、`tests38::the_messages`、`ui/tests/connect-components.test.ts` の「知らせ」の行。
