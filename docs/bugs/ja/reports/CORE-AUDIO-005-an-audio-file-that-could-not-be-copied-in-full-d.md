---
id: CORE-AUDIO-005
title: 音のファイルを入れられなかった（ディスクがいっぱい、作業の場所がない）のに、読めない種類だと知らされる
area: CORE-AUDIO
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# CORE-AUDIO-005: 音のファイルを入れられなかった（ディスクがいっぱい、作業の場所がない）のに、読めない種類だと知らされる

## Where

crates/meloet-core/src/audiofiles/keeper.rs:392-393 (`Work::bring`: `bring_in` の失敗が `Unread::NotAudio` になる)
crates/meloet-core/src/audiofiles/keeper.rs:418-420, 431-441 (`Work::found`: 作業の場所がない、またはコピーが失敗すると `Finding::Unreadable` になる)
crates/meloet-core/src/state/audio.rs:245, 291 (`Unreadable` / `NotAudio` が `StatusMessage::AudioUnreadable` になる)

## What happens

ファイルを入れるときは、まずそれが何かを確かめ（`decode::facts`）、次に作業の場所へコピーします（`AudioStore::bring_in`）。コピーが失敗すると（ディスクがいっぱい、作業の場所に書けない、状態のフォルダがない）、keeper は `Brought::Unreadable(name, Unread::NotAudio)` を返し、状態行には、正しく読めている wav ファイルについて「「声.wav」は読めません（読めるのは wav ・ mp3 ・ flac ・ ogg）。」と出ます。「さがす…」も、コピーが失敗したときは同じになります（`Finding::Unreadable` が同じ文言になる）。使う人は別の種類のファイルを使うように言われますが、それでは直りません。本当の原因（空きがない、書く権利がない）は、標準エラー出力にしか出ません。

## What should happen

知らせの文言は原因と合っている必要があります（docs/bugs の指示: 誤解を招く文言は書かない）。`facts` はすでに「meloet が読まない種類」（`NotAudio`）、「壊れている」（`Broken`）、「チャンネルが多すぎる」を区別しています。書けないことは、そのどれでもありません。仕様 6 には、保存できない曲について、ディスクを名指しする文言があります（「ここには保存できませんでした（書きこむ権利がないか、空きが足りません）。…」）。しかし、入れられない音のファイルについては、そうした文言がありません。

## How to trigger

コードを読んで確認したもので、実行はしていません。

1. `~/.local/state/meloet` があるディスクをいっぱいにする（または `work/音` に書けないようにする）。
2. 正常な wav ファイルをアレンジにドラッグする。
3. 状態行に、そのファイルは読めず、読めるのは wav だと出る。

## Cause

`Work::bring` は、`store.bring_in`（作業の場所へのコピー）のすべてのエラーを `Unread::NotAudio` に対応させ、`Work::found` は、コピーが失敗したとき `Finding::Unreadable` を返します。`Unread` には「コピーできなかった」という場合がありません。

## Suggested fix

`Unread`（と `Finding`）に「作業の場所に書けなかった」の場合を加え、それ専用の知らせを出す。新しい文言（リードかオーナーが決める）と、新しい `StatusMessage` の種類（プロトコルの追加）が必要です。既存の文言はどれも別の原因を書いているので、新しい文言がなければ小さく直す方法はありません。

## Verification

実際にあるもので、コードを読んで確認しました（テストは実行していません）。`Work::bring`（`audiofiles/keeper.rs:389-393`）は、まずファイルの情報を読み、次に `store.bring_in`（作業の場所へのコピー）のすべてのエラーを `Unread::NotAudio` に対応させます。`Work::found`（`keeper.rs:414-442`）は、作業の場所がないか、コピーが失敗すると `Finding::Unreadable` を返します。`state/audio.rs:245, 291` は両方を `StatusMessage::AudioUnreadable` に変えます。その文言は、正しく読めるファイルについて「「声.wav」は読めません（読めるのは wav ・ mp3 ・ flac ・ ogg）。」となります。

既存の文言を探しました。`AudioUnreadable`、`AudioBroken`、`AudioTooManyChannels`、`AudioFoundMismatch` は、それぞれ別の原因を書いています。`ClipWorkFailed`（「{count} つのクリップは、音のファイルを読めないか書けないので、そのままにしました。」）は、すでに曲の中にあるクリップのことです。`msg.bounceFailedWrite` と `msg.exportFailedWrite` は、それぞれ別の作業についてのものです。「ファイルを入れられなかった」と言うものはありません。

`needs-lead` にした理由: 新しい文言と、新しい `StatusMessage` の種類（プロトコルの追加）が必要で、さらに `Unread` と `Finding` に新しい場合を加えるからです。選択肢:
1. 新しい種類（たとえば `AudioNotCopied { file }`）を、オーナーの文言とともに作る。たとえば「「声.wav」を入れられませんでした。作業の場所に書けません。」（2 文目は `msg.bounceFailedWrite` のもの）。これを推奨します。CORE-AUDIO-003 と -004 も同じ種類の文を必要とするので、一緒に決めるのが最善です。
2. そのままにして、`docs/progress.md` に書き出す。

重大度 `low` はそのままです（ディスクがいっぱい、または作業の場所に書けないとき）。

## Fix

プロトコル 38（リードの決定）: `StatusMessage::AudioNotCopied { file }`。`Unread::NotCopied`（`crates/meloet-core/src/audiofiles/decode.rs`）と `Finding::NotCopied`（`audiofiles/keeper.rs`）。`Work::bring` は `bring_in` が失敗したとき、`Work::found` は作業の場所がないか、コピーが失敗したときにそれを返し、`crates/meloet-core/src/state/audio.rs` は両方を `AudioNotCopied` に変えます。文言は `msg.audioNotCopied`（`builder:2026-11-25`）、`StatusLine.svelte` に置きます。テスト: `audiofiles::tests::a_file_that_cannot_be_copied_in_says_so`、`state::audio::tests::files_that_do_not_read_bring_nothing_in` の新しい行、`state::audio::tests::a_missing_file_is_said_looked_for_and_taken_out` の新しい手順、`tests38::the_messages`、`ui/tests/connect-components.test.ts` の「知らせ」の行。
