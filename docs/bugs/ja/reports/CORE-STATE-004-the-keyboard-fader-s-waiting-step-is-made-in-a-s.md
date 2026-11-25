---
id: CORE-STATE-004
title: 鍵盤のフェーダーの待っている手が、動かした曲を置き換えた後の曲で作られる
area: CORE-STATE
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-STATE-004: 鍵盤のフェーダーの待っている手が、動かした曲を置き換えた後の曲で作られる

## Where

crates/meloet-core/src/state/buttons.rs:110-115 (`keyboard_fader` は `Pending { track, db, at_us }` をトラックの id だけで持つ)
crates/meloet-core/src/state/buttons.rs:128-146 (`fader_due` / `commit_fader` は、その id に `SetGain { commit: true }` を送る。そのとき入っている曲が何であっても)
crates/meloet-core/src/state/songs.rs:536-541 (`replace_song` は `self.fader` を手放さない)

## What happens

鍵盤のフェーダー（CC 7）は、選ばれたトラックの音量を試聴として設定し、その手は、フェーダーが止まってから 0.5 秒後に作られます（`FADER_REST_US`）。その半秒の間に曲が置き換えられると（「新しい曲」、または「最近ひらいた曲 ▸」を、未保存の変更がない状態で。PC が渡したファイル。AI の接続の新しい曲や開く曲）、試聴は古い曲と一緒に消えます（`Play::replace_song` → `forget_mixer`）。しかし、待っている手は残ります。次の `Input::Tick` で、それは新しい曲の中で作られます。そこで同じ id を持つトラック（トラックの id は曲ごとに数え直されるので、たいていはトラック 1）が、古いフェーダーの音量の「音量を変える」手を受け取ります。新しい曲は変更されたと表示され、音量は、誰も触っていないのに飛びます。

## What should happen

手は、それが作られた曲に属するべきです。docs/architecture.md 13: 「Esc（`CancelMixPreview`）、別の手、取り消し、曲の置き換えで試聴は終わる」。フェーダーの音量は、止まるまでの試聴なので、曲を置き換えると試聴は終わるべきで、次の曲の手になってはいけません。

## How to trigger

コードを読んで確認したもので、実際には動かしていません。
1. 未保存の変更のない曲で、トラック 1 を選びます。鍵盤のフェーダーを動かします（`Input::KeyboardFader`）: `fader.pending = Some(Pending { track: 1, .. })`。
2. 0.5 秒以内に「新しい曲」を選びます（確認なし、未保存の変更なし）: `then(Then::NewSong)` が新しい曲を置きます。
3. フェーダーの最後の動きから 0.5 秒後に、`Input::Tick` → `fader_due` → `commit_fader` → `SetGain { strip: Track { id: 1 }, commit: true }` → `Play::mix` が、新しい曲のトラック 1 に手を作ります（試聴がなくても手を作ります。play/mixer.rs:251-260）。

## Cause

`Model::fader`（`buttons::Fader`）は曲と結びついていません。`replace_song` も他の何も、別の曲が入れ替わったときに `pending` を消したり、連番を比べたりしません。

## Suggested fix

`Pending` に曲の連番を持たせ、`self.play.serial()` と違うときは `commit_fader` で待っている手を捨てる。または `Model::replace_song` で `self.fader` を既定値に戻す。プロトコル、文面、曲のファイルの形式の変更はありません。

## Verification

コードを読んで確認しました（テストは実行していません）。`keyboard_fader`（`state/buttons.rs:110-115`）は `Pending { track, db, at_us }` を保ちます。`fader_due` / `commit_fader` は、半秒後に `SetGain { strip: Track { id }, commit: true }` を送ります。`Model::replace_song`（`state/songs.rs:537-541`）は `self.fader` に触れません。トラックの id は曲ごとに数え直されるので、手は新しい曲の、同じ id のトラックに入ります。重大度 `low` のままです（フェーダーを動かしてから 0.5 秒以内に曲を置き換える必要があります）。

訂正はありません。

修正の計画: `crates/meloet-core/src/state/songs.rs` の `Model::replace_song`: `self.fader = Default::default();`（`Fader` に曲をまたいで残すべきもの、例えば拾い直しの位置があるなら、`pending` と `set` だけを消す）。テスト（`state/buttons_tests.rs` か、フェーダーのテストのそば）: フェーダーを動かし、曲を置き換え、0.5 秒後に tick → 新しい曲に手がないこと。

## Fix

`crates/meloet-core/src/state/songs.rs` の `Model::replace_song`: `self.fader = Default::default()`。そのため、鍵盤のフェーダーの待っている手（とそれが捕まえたもの）は、動かされた曲と一緒に消え、新しい曲では作られません。

テスト: `state::songs::tests::a_keyboard_fader_step_still_waiting_goes_with_the_song_it_was_for`（`state/songs/tests.rs`）。
