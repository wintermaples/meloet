---
id: CORE-AUDIO-002
title: 歌のファイルは、鍵盤のように 1 拍の半分ではなく、4 分音符の半分だけ前から始まる。6/8 と 2/2 では、歌と鍵盤の窓がずれる
area: CORE-AUDIO
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-AUDIO-002: 歌のファイルは、鍵盤のように 1 拍の半分ではなく、4 分音符の半分だけ前から始まる。6/8 と 2/2 では、歌と鍵盤の窓がずれる

## Where

crates/meloet-core/src/service/input.rs:407-411 (`vocal_mark`: `half_beat_us = 30e6 / map.tempo_at(tick)`)
crates/meloet-engine/src/engine.rs:2712 (合わせるべき鍵盤の決まり: `t.record_from - t.record_beat / 2`。`record_beat` は `record_from` の `Meter::ticks_per_beat()`、engine.rs:1130-1132)

## What happens

ディスクへの書き出し役には、曲が始まる瞬間より `half_beat_us` 前から歌のファイルを始めるように伝えられる。`tempo_at` は 1 分あたりの 4 分音符の数である (docs/architecture.md 22-2)。そのため `30e6 / tempo` マイクロ秒は、拍子に関係なく、いつも 4 分音符の半分 (960 ティック) になる。

鍵盤は、カウントインの最後の半拍に弾いたものを残す。ここでの 1 拍は、下の数字の音符である (`Meter::ticks_per_beat` = 7680 / 分母)。x/4 では 960 ティック、x/8 では 480 ティック、x/2 では 1920 ティックになる。

- 2/2 (ほかの x/2 も) では、歌のファイルは曲が始まる前の 8 分音符分しか始まらないが、鍵盤は 4 分音符分を残す。4 分音符から 8 分音符の間だけ早く歌い出した弱拍は、ファイルに入らないので、クリップの左の端を外に引き出すと、そこが無音に見える。
- 6/8、3/8、x/16 では、歌のファイルは鍵盤の 2 倍 (4 倍) 残す。

その行の上のコメントは「曲が始まる前の半拍。鍵盤が残すのと同じ」と書いている。

## What should happen

docs/architecture.md 22-6: 「ファイルは、カウントインの最後の半拍の先頭から書かれる (鍵盤の決まり 12 と同じ長さ)」。22-7 と 22-16 も同じことを書いている (「カウントインの最後の半拍から」)。鍵盤の決まりは `Meter::ticks_per_beat()` の半分である (engine.rs:2712)。

## How to trigger

コードを読んで分かったこと。実行はしていない。

1. 拍子を 2/2、テンポを 120 にする (4 分音符は 0.5 秒)。
2. 歌のトラックを待機にし、カウントインつきで録音し、最初の小節の 0.4 秒前から歌い始める (8 分音符から 4 分音符の間だけ早い)。
3. 停止し、クリップの左の端を始まりより前へ引き出す。弱拍の前の部分はない (ファイルは曲が始まる 0.25 秒前から始まる)。同じ瞬間に弾いた鍵盤は残っている。

## Cause

弱拍の半拍は、テンポだけから計算されている。テンポが、どの拍子でも 1 分あたりの 4 分音符になったあと (22-2)、この行は今も「4 分音符の半分」を意味し、エンジンの `record_beat` のように、録音の始まりの拍子とは結び付けられていなかった。

## Suggested fix

エンジンと同じく、曲が始まるティックの拍子から半拍を取る: `let beat = map.meter_at(tick.max(0.0) as u64).meter.ticks_per_beat() as f64;` のあと、`half_beat_us = beat / 2.0 / TICKS_PER_QUARTER as f64 * 60e6 / map.tempo_at(tick).max(1.0)`。プロトコル、ファイル、文言は変えない。

## Verification

コードを読んで確認した (テストは実行していない)。`vocal_mark` (`service/input.rs:407-411`): `half_beat_us = 30e6 / map.tempo_at(tick)`。テンポは拍子に関係なく 1 分あたりの 4 分音符なので、これはいつも 4 分音符の半分になる。エンジンの鍵盤の決まりは `record_from - record_beat / 2` で、`record_beat = Meter::ticks_per_beat()` = `TICKS_PER_QUARTER * 4 / den` である (`engine.rs:1130-1132`、`2712`)。アーキテクチャ 22-6: 「鍵盤の決まりと同じ長さ」。x/2 では歌のファイルは、鍵盤が残す半分の早さでしか始まらず、x/8 では 2 倍早く始まる。重大度 `low` は変えない (下の数字が 4 でない拍子で、クリップの始まりより前の部分だけ)。

訂正: なし。

修正案: `crates/meloet-core/src/service/input.rs` の `vocal_mark`: 曲が始まるティックの拍子から拍を取る: `let beat = map.meter_at(tick.max(0.0) as u64).meter.ticks_per_beat() as f64; let half_beat_us = beat / 2.0 / TICKS_PER_QUARTER as f64 * 60e6 / map.tempo_at(tick).max(1.0);`。小さな計算の単体テスト (関数に分ける): 同じテンポで 4/4、2/2、6/8。

## Fix

`crates/meloet-core/src/service/input.rs`: `vocal_mark` は、新しい `half_beat_us(map, tick)` から半拍を取る。その場所のテンポで、曲が始まるティックの拍子の `Meter::ticks_per_beat()` の半分 (エンジンの鍵盤の決まり)。変えたのは `vocal_mark` のその行だけ。テスト: `service::input::tests::the_files_begin_half_a_beat_of_the_meter_before_the_roll` (120 で 4/4、2/2、6/8。修正がなければ失敗する)。
