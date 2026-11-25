---
id: CORE-STATE-003
title: 保存が書かれている間に、ウィンドウを閉じるか終了すると、何も知らせずに無視される
area: CORE-STATE
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# CORE-STATE-003: 保存が書かれている間に、ウィンドウを閉じるか終了すると、何も知らせずに無視される

## Where

crates/meloet-core/src/state/songs.rs:454-457 (`close_requested`: `if self.files.exiting || self.files.waiting.is_some() { return; }`)
crates/meloet-core/src/service.rs:3716-3785 (`save_song`: プラグインや音声のある曲は `meloet-save` スレッドで書かれ、答えは後から来る)

## What happens

プラグイン(その状態を待つ最大 `STATE_WAIT` = 3 秒)や音声(`音/` に音声をコピーするため、状態行に「保存しています ・ 音のファイルを写しています …」と出て、何秒もかかることがある)のある曲が書かれている間は、ウィンドウの閉じるボタンも「meloet を終わる」(`Command::Quit`)も何もしません。その要求は捨てられます。保存が終わると meloet は開いたままで、使う人は、最初の操作が何もしなかった理由が分からないまま、もう一度閉じることになります。

## What should happen

閉じる要求は、録音中と同じように残しておき、曲が空いたあとに行うべきです。docs/architecture.md 14 には「閉じる要求は録音を止め、録ったものを曲に入れてから決める」(`close_when_recorded`、songs.rs:447-452)とあります。`Waiting` の doc コメント(songs.rs:37)も「そのあいだ、曲の命令は待つ」と書いており、捨てるとは書いていません。保存が答えたあとは、いつもの流れが続きます。保存していない変更がなければ終わる。保存中に変更があれば「保存していない変更があります」。

## How to trigger

コードを読んで確認したもので、実行はしていません。
1. 音声クリップ(またはプラグイン)のある曲を開き、Mod S を押す: `Effect::SaveSong` が保存スレッドを始め、`files.waiting = Some(Waiting::Save(Stay))` になる。
2. 状態行にまだ「保存しています …」と出ている間に、ウィンドウの閉じるボタン(`Input::CloseRequested`)を押すか、「meloet を終わる」を選ぶ。
3. 何も起きない。その後、`Input::SongSaved` が来ても、何も起きない。

## Cause

`close_requested` は、`files.waiting` が設定されていると、すぐに戻ります。その要求を覚えておくものはありません。すぐ下の場合(録音中)は、`files.close_when_recorded` に要求を残すのとは違います。

## Suggested fix

`files.waiting` が `Some(Waiting::Save(_))` のとき、印(新しい印、または `close_when_recorded` を再利用)を立て、保存が答えたあとに `close_requested` をもう一度呼びます(`song_input` の `SongSaved` / `SongNotSaved`。`SongNotSaved` のあとは、「保存できない」ダイアログが先に出るかもしれません)。プロトコル、文面、ファイルの形は変えません。

## Verification

コードを読んで確認しました(テストは実行していません)。`close_requested`(`state/songs.rs:454-457`)は、`files.waiting.is_some()` のとき、すぐに戻り、要求を保持するものがありません。録音中の場合は、その下で `files.close_when_recorded` に保持されます。プラグインや音声のある曲の保存は、後から答えます(`service.rs` の `save_song` は自分のスレッドで動く)ため、その間の閉じる、または「meloet を終わる」は失われます。重大度 `low` は変えません(回避策は、もう一度閉じることです)。

訂正: `files.waiting` は、開く操作やほかのファイルの操作が待つときにも設定されます。以下の修正は保存のためのもので、ほかの操作の間の閉じる要求も、追加の費用なしに同じように残せるなら、残してもよいです。

修正案: `crates/meloet-core/src/state/songs.rs`: `close_requested` で、待っているものがあるとき、印(新しい `close_when_free`、または、保存の答えのあとにも読まれるなら `close_when_recorded` を再利用)を立てて戻る。`SongSaved` / `SongNotSaved`(および `files.waiting` を消す他の答え)が処理されるところで、印が立っていて、答えからダイアログが出なければ `close_requested` をもう一度呼ぶ(`SongNotSaved` のあとは、「保存できない」ダイアログが先に出て、印は捨てられる)。テスト `state/songs/tests.rs`: 保存が待っている間に `CloseRequested`、それから `SongSaved` → `Effect::Exit`。

## Fix

`crates/meloet-core/src/state/songs.rs`: 新しい印 `Files::close_when_saved`。保存が書かれている間に `close_requested` が立てます。保存が答えたとき(`SongSaved`)に、閉じる処理をそこで行います(保存のあとに続くはずだった新しい曲や開く操作より前。すでに続いたものが閉じる処理なら、ただ終わります)。`SongNotSaved` のあとは、「保存できない」が先に出て、印は捨てられます(ダイアログを出さない AI リンクの保存では、閉じる処理は続きます)。新しい文面はなし。`docs/architecture.md` 14(流れ)に、そう書きました。

テスト: `state::songs::tests::closing_while_a_save_is_written_closes_once_it_has_answered`(`state/songs/tests.rs`: 保存の間に閉じると「meloet を終わる」、それから答えが来て meloet が終わる。保存の間の変更は、確認を求められる。保存が失敗したら「保存できない」が出て、閉じる要求はそれを越えて残らない)。
