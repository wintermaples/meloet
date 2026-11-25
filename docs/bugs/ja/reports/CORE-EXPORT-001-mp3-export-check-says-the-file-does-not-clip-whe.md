---
id: CORE-EXPORT-001
title: wav で 32 ビットを選んだ後に mp3 にすると、書き出しの確認で音割れなしと出る
area: CORE-EXPORT
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-EXPORT-001: wav で 32 ビットを選んだ後に mp3 にすると、書き出しの確認で音割れなしと出る

## Where

crates/meloet-core/src/export.rs:919 (`check`: `let clips = over > 0 && self.bits != RecordFormat::Bits32 && !matched;`)
crates/meloet-core/src/export.rs:524-531 (`ExportChange::Format`: 新しい形式にビット数がないとき、ビット数は保たれる)
crates/meloet-core/src/export/sink.rs:384-391 (`Mp3Sink::push` は、すべてのサンプルを -1.0 から 1.0 に収める)

## What happens

書き出しの画面の確認は、`clips` を `self.bits` だけで決めます。使う人が「くわしく」で「32 ビット」を wav に選んでから、形式を mp3 に変えると、mp3 にはビット数がない（他の形式のために保たれる）ので、`self.bits` は `Bits32` のままです。そのため、render が 0 dB を超える mp3 では、確認は `clips: false` を返します。画面には、音割れの行のない「いちばん大きい所 … ・ 音割れなし」の文だけが出て、ボタンは「このまま書き出す」ではなく「書き出す」になります。しかし、mp3 のシンクはすべてのサンプルを最大値に収めるので、書かれた mp3 は `over` に数えられたすべての所で割れています。使う人は、ファイルは問題ないと伝えられ、割れたファイルを受け取ります。

## What should happen

`ExportCheck::Done::clips` は「`bits` で書いた場合のファイルで、音が割れるかどうか」です（プロトコル `crates/meloet-protocol/src/export.rs`、`ExportCheck` の説明。仕様 6「書き出しで音が割れる」）。mp3 は常に満量以内で書かれる（sink.rs、`Mp3Sink::push` のコメント「ファイルには、プレーヤーが再生できるものを入れる」）ので、mp3 では、wav に残したビット数に関係なく、`clips` は `over > 0`（一致する場合を除く）でなければなりません。0 dB を超えるサンプルを残せるのは、32 ビットの浮動小数点の wav だけです。

## How to trigger

コードを読んで確認したもので、実際には動かしていません。
1. 「書き出し…」を、マスターが 0 dB を超える曲で開きます（例: フェーダーを +6 dB にした一つのトラック）。
2. 形式を wav にし、「くわしく」を開き、「32 ビット」を選びます。
3. 形式を mp3 に変えます。確認が終わるまで待ちます。
4. 確認には音割れがなく、ボタンは「書き出す」と出ます。書かれた mp3 は割れています。
（単体テスト `a_matched_file_does_not_clip_and_its_gain_can_be_down` は wav だけを対象にしています。）

## Cause

`check` は `self.bits` を見て、`self.format` を見ません。`ExportChange::Format`（export.rs:524-531）は、`format.bits_choices()` が空（mp3、MIDI）の場合 `self.bits` を保ちます。仕様 3 F2（「wav と flac で選んだビット数は、そのまま残る」）が求めるとおりです。そのため、wav で選んだ `Bits32` が、形式が mp3 の間も保たれたままになります。

## Suggested fix

`check` で、ビット数を浮動小数点として数えるのは、その形式がそれを受け付ける場合だけにします。例:
`let floats = self.format == ExportFormat::Wav && self.bits == RecordFormat::Bits32;`
`let clips = over > 0 && !floats && !matched;`
（flac は Bits32 を持てません。`ExportChange::Format` がそれをリセットします。）プロトコル、文面、見た目の変更はありません。テスト: wav で 32 ビットにし、次に mp3 にして、`over > 0` の render で `clips: true` になること。

## Verification

コードを読んで確認しました（テストは実行していません）。`check`（`export.rs:919`）: `clips = over > 0 && self.bits != RecordFormat::Bits32 && !matched`。`self.format` は見ません。`ExportChange::Format`（`export.rs:524-531`）は、新しい形式にビットの選択肢がない（mp3、MIDI）場合 `self.bits` を保つので、wav で選んだ `Bits32` が、形式が mp3 の間も残ります。`Mp3Sink::push`（`export/sink.rs:380-391`）は、すべてのサンプルを ±1.0 に収めます。そのため、確認は、割れて書かれる mp3 について「音割れなし」と言います。重大度 `medium` のままです（使う人はファイルは問題ないと伝えられます。回避策は wav を 24 ビットに戻すことです）。

訂正はありません。

修正の計画: `crates/meloet-core/src/export.rs` の `check`: `let floats = self.format == ExportFormat::Wav && self.bits == RecordFormat::Bits32; let clips = over > 0 && !floats && !matched;`（名前を wav にするより、`self.format.bits_choices().contains(&RecordFormat::Bits32)` と書くほうが読みやすければそちらにします）。そのとき、`self.bits` の他の読み手で、形式を忘れているものも確認します（完了の文面、仕事のビット数）。テスト（`export/tests.rs` か `mp3_tests.rs`）: wav で 32 ビット、次に mp3、`over > 0` の render → `clips: true`。

## Fix

`crates/meloet-core/src/export.rs` の `check`: ビット数は、形式が 32 ビットを受け付ける場合（`self.bits == Bits32 && self.format.bits_choices().contains(&Bits32)`）にだけ浮動小数点として数えます。そのため、wav に残したビット数に関係なく、over > 0 の mp3 は割れると判定されます。`self.bits` の他の読み手も確認しました。仕事のビット数と完了の画面はビット数を持ちますが、mp3 のシンクはそれを読まず、完了の画面は mp3 にビットレートを出します。`docs/architecture.md` の 23-7 と 23-8 は「32 ビットの wav」と書いています。

テスト: `export::tests::an_mp3_clips_whatever_bits_were_kept_for_wav`（`export/tests.rs`）。
