---
id: CORE-PLAY-A-004
title: さっきのを残す: 4096 個を超えるイベントがあるフレーズで、1 つの鍵の離す動作が捨てられ、音がフレーズの終わりまで伸びる
area: CORE-PLAY-A
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-PLAY-A-004: さっきのを残す: 4096 個を超えるイベントがあるフレーズで、1 つの鍵の離す動作が捨てられ、音がフレーズの終わりまで伸びる

## Where

- crates/meloet-core/src/play/recent.rs:156-164 (`Phrase::push`: `notes <= 1` のとき `self.events.remove(1)`)

## What happens

「さっきのを残す」 のフレーズに、鍵を押した 1 回と、その後にそれ以外のイベント 4095 個を超えるもの (ピッチベンド、モジュレーションホイール、CC を送るつまみ) があると、いっぱいになるたびに `events[1]` が捨てられます。鍵がもう離されていれば、`events[1]` はその鍵の離す動作 (NoteOff、または速度 0 の NoteOn) なので、最初に捨てられるのは離す動作です。残されたクリップでは、その音はフレーズの終わりまで押さえられたままになります (`keep_played` は、すべての鍵を `placed.end` で離す。recent.rs 465-471)。たとえば短い音が、長い音になってしまいます。

## What should happen

recent.rs 160-161 のコメントに規則が書かれています: 「長く続くコントローラーの動きの間ずっと押されている 1 つの鍵: 鍵は残り、動きのうち一番古いものが捨てられる。」 捨ててよいのはコントローラーの動きだけです。鍵自身の離す動作は残らなければならず、そうしないと音は弾いたときの長さを保てません (アーキテクチャ 25-16: ノートの全体は古い側から捨てる。半分に切ることはしない)。テスト `one_key_held_under_thousands_of_controller_moves_stays` (recent/tests.rs 590) は、鍵が押されたままの場合だけを扱っています。そのとき `events[1]` はコントローラーの動きです。

## How to trigger

コードを読んだ結果です。実行はしていません。静止した状態で、C4 を押して離す (2 つのイベント)。その後、ピッチベンドかモジュレーションホイールを数秒動かす (4095 個を超えるメッセージ。ホイールは動いている間、約 1 ms ごとに 1 つ送る)。「さっきのを残す」 を押すと、クリップの C4 は、押した時点からフレーズの終わりまで続く。テストとしては、`still(0, S, on(60))`、`still(0, S + 1000, off(60))`、その後に `Cc { number: 1 }` を 5000 個、残す。すると、音の長さは弾いた 1000 µs ではなくなる。

## Cause

`notes <= 1` のとき、コードは何のイベントかを確かめずに index 1 を取り除きます。1 回押した後、index 1 は次に来たものです。鍵が離されると、それが離す動作になります。

## Suggested fix

コントローラーの動きのうち一番古いものを、index 1 ではなく、捨てる。index 1 以降 (`i >= 1`) で最初の `Cc` または `PitchBend` を見つけて取り除く。見つからないときだけ `remove(1)` に戻す (途中のコントローラーの動きを捨てるのは今のコードでも行っているので、`head` は変更不要)。プロトコル、ファイル、文言の変更はありません。

## Verification

コードを読んで確認しました (テストは実行していません)。`Phrase::push` (`play/recent.rs:152-169`): フレーズがいっぱいで、鍵を押したイベントが 1 つ以下のとき、イベントの種類を見ずに `self.events.remove(1)` を呼びます。押した動作が index 0 にあり、その離す動作がすぐ後にあるとき、最初に捨てられるのは離す動作です。その後 `keep_played` は、その鍵をフレーズの終わりで止めます。コメントには、捨ててよいのはコントローラーの動きだけだと書かれています。重大度 `low` はそのままです (1 つの鍵と、1 つのフレーズの中の 4095 個を超えるコントローラーの動き)。

Corrections: 訂正なし。

修正の計画: `crates/meloet-core/src/play/recent.rs` の `Phrase::push`: `else` の分岐で、index 1 以上にある最初の `Cc` または `PitchBend` のイベントを取り除く。見つからないときだけ `remove(1)` に戻す。テスト: `recent/tests.rs` の `one_key_held_under_thousands_of_controller_moves_stays` の隣に、押す、1000 µs 後に離す、CC の動きを 5000 個、残す。音は弾いた長さを保つ。

## Fix

`crates/meloet-core/src/play/recent.rs` の `Phrase::push`: 鍵を押した 1 回だけがあるいっぱいのフレーズでは、押した後の一番古いコントローラーの動き (`Cc` または `PitchBend`) が捨てられる。そのようなものがないときだけ、これまでどおり押した後のイベントが捨てられる。

テスト (`play/recent/tests.rs` 内): `one_key_let_go_under_thousands_of_controller_moves_keeps_its_length`。
