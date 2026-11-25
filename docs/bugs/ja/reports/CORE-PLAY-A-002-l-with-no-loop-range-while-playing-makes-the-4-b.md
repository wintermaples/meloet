---
id: CORE-PLAY-A-002
title: 再生中に、ループの範囲がない状態で L を押すと、4 小節の範囲は再生の線ではなく、再生を始めた小節にできる
area: CORE-PLAY-A
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-PLAY-A-002: 再生中に、ループの範囲がない状態で L を押すと、4 小節の範囲は再生の線ではなく、再生を始めた小節にできる

## Where

- crates/meloet-core/src/play.rs:991-996 (`Play::toggle_loop`: `map.meter_at(self.position)`)
- crates/meloet-core/src/play.rs:490-491 (`position`: 「動いている間は、始まった所」)

## What happens

曲にループの範囲がないとき、再生中に L を押す (または、キーボードの Cycle / Middle のボタン、`state/buttons.rs` 84) と、範囲は、再生の線のいる小節ではなく、再生を始めた (または最後に移動した) 小節から 4 小節になる。1 小節から再生し、20 小節で押すと、ループは 1〜5 小節になる。エンジンはそのループの終わりを過ぎているので、周回しない (`engine.rs` の `SetLoop`: `before_loop_end` が false になる)。目盛りには、再生の線から遠く離れたループが表示され、次の Space で 1 小節目へ戻る (再生の線がループの外にあるとき、`play_stop` はループの頭から始める)。

## What should happen

仕様 3 (3-bands.md 52 行目): 「範囲がないときにループを入れると、今の位置の小節から 4 小節になる。」 再生中の今の位置は、再生の線 (使う人に聞こえているもの) である。このファイルで、動いている間に「再生の線の位置」で働く、ほかのすべての命令は `Play::play_line()` (play.rs 961-970) を使う: `paste_clips` (1242)、`set_tempo` と `set_meter` (timeline.rs 197、226)、`add_marker` (timeline.rs 378)、`step_marker`、`open_piano_roll`、`place_pattern`、`bar_step`。

## How to trigger

実行して確かめたのではなく、コードを読んで確かめた。
1. ループの範囲がない新しい曲を作る。1 小節目で Space を押す。
2. 再生の線が 20 小節にあるとき、L を押す。
3. ループは 1〜5 小節になる (`EngineOp::Loop { start: 0, end: 4 * bar }`)。曲はループを過ぎても再生を続ける。
テスト `the_loop_turned_on_with_no_range_takes_four_bars_from_the_current_one` (play/tests.rs 323) は、止まっている場合だけを調べる。そこでは `position` が再生の線である。

## Cause

`toggle_loop` は `self.position` を読む。再生中の position は、再生が始まった tick (または最後の `Locate`) であり、聞こえている位置ではない。

## Suggested fix

play.rs 994 の `self.position` の代わりに `self.play_line()` を使う (`let head = map.meter_at(self.play_line());`)。止まっているとき、`play_line()` は `position` なので、止まっている場合は変わらない。プロトコル、ファイル、文言は変えない。

## Verification

コードを読んで確かめた (テストは実行していない)。`Play::toggle_loop` (`play.rs:986-1000`) は `map.meter_at(self.position)` を使う。`position` は「再生と録音が始まる所。動いている間は、始まった所」である (`play.rs:490-491`)。`play_line()` (`play.rs:961-970`) は、ほかの「再生の線の位置」の命令が使うものである。仕様 3 の 52 行目: 「今の位置の小節から 4 小節」。`7-readings.md` で、これを開始位置と読むところはない。既存のテストは、止まっている場合だけを扱う。重大度 `medium` は変えない。

訂正: なし。(ループが再生の線の後ろで終わるとき、エンジンが何をするかは、私は実行していない。この不具合の原因には、`SetLoop` の読みは必要ない。)

修正計画: `crates/meloet-core/src/play.rs` の `toggle_loop`: `map.meter_at(self.play_line())`。テストは `crates/meloet-core/src/play/tests.rs` の `the_loop_turned_on_with_no_range_takes_four_bars_from_the_current_one` の隣に置く。再生中で、聞こえている位置が 20 小節にあるとき、L を押すと 20〜24 小節になる。

## Fix

`crates/meloet-core/src/play.rs` の `toggle_loop`: 4 小節は、`play_line()` の小節から始まる (動いている間に聞こえているもの。止まっているときは `position`)。

テスト (`play/tests.rs` の中): `the_loop_turned_on_with_no_range_while_playing_takes_the_bar_of_the_play_line`。
