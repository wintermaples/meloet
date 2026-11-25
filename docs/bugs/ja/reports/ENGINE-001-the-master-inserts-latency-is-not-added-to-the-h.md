---
id: ENGINE-001
title: マスターのインサートの遅れが耳に届く位置に足されていない: 再生の線、録音した鍵、メトロノームがその分ずれる
area: ENGINE
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# ENGINE-001: マスターのインサートの遅れが耳に届く位置に足されていない: 再生の線、録音した鍵、メトロノームがその分ずれる

## Where

crates/meloet-engine/src/engine.rs:2729-2731 (`dac_us`: `playback_us + frame / rate`。マスターのインサートの分は何も足していない)
crates/meloet-engine/src/engine.rs:2739-2758 (`segment_now`: どの時計の区間も `dac_us` から始まる)
crates/meloet-engine/src/engine.rs:1808-1811 (先読みが見ているのはトラックの音源とインサートだけ)
crates/meloet-engine/src/engine.rs:1643-1648 (`master_latency` を読むのは書き出しだけ: crates/meloet-core/src/export/job.rs:501, 820)
crates/meloet-engine/src/engine.rs:3084-3110 (メトロノームのクリックはマスターのインサートの後に足される。そのため曲と一緒に遅れない)

## What happens

マスターのインサートが音を遅らせると (遅れを報告するプラグイン、たとえば先読みのあるリミッターや位相直線の EQ など)、マスターを通るものはすべて、エンジンの時計が示す位置より L フレーム遅れて耳に届きます。再生中はこれを補う処理がありません。

- 時計の区間は、区間の 0 フレーム目に曲の位置が `playback_us` で DAC に届くと示します。しかし、その時点で人が聞いているのは曲の L フレーム前の音です。画面の再生の線 (テレメトリの `heard_now`) は、音より L だけ先に進みます。
- 曲に合わせて人が弾いた鍵には、時計がその時刻に示す位置 (`stamp`、`stamp_recent`、`stop` の止めた位置) が付きます。それは人が聞いた位置より L だけ後ろです。そのため、録音した鍵や残した鍵は L だけ遅れて出ます。歌の録音も同じ時計 (`frame_at`、`time_of_frame`) を使うので、曲に合わせて歌った声も L だけ遅れて置かれます。
- メトロノームはマスターの後に足されます (マスターのストリップを飛ばします)。そのため、クリックは曲の対応する拍より L だけ早く鳴ります。

先読みのリミッターが 5 ms なら、48 kHz で 240 フレームです。位相直線の EQ では数十 ms になることもあります。

## What should happen

docs/architecture.md 20-8: 「マスターのインサートの遅れは全部同じ。補正はしないが、耳に届いている位置 (第 11 章) に足す。」 (日本語の原文 docs/history/architecture.ja.md:841 も同じことを書いています: マスターのインサートの遅れは … 「耳に届いている位置」に足す.) 書き出しは、先頭の `master_latency()` フレームを切ることでこれを扱っています。ライブの時計はそうしていません。

## How to trigger

コードを読んだ結果です。実行はしていません。

1. 遅れを報告するエフェクト (`getLatencySamples` または `lv2:latency` が 0 でないプラグイン。先読みのリミッターなど。この PC のどのプラグインがそうかは確認していません) をマスターに置く。
2. メトロノームをオンにして再生する。クリックは、曲の拍より、プラグインの遅れ分だけ前に鳴る。
3. 曲に合わせて鍵を録音する。ノートは同じ分だけ遅れて入る。再生の線は、聞こえている音より先に進んでいる。

## Cause

耳に届く位置の時計は、cpal の `playback` 時刻とフレーム数 (`dac_us`) だけから作られています。マスターのストリップの `Strip::latency()` は `Engine::master_latency()` でだけ合計され、それを読むのは書き出しだけです。トラックの遅れは、ノートを早めに読むこと (`lookahead`) で補われていますが、マスターの遅れは全トラックに共通なのに、ライブの経路のどこにも足されていません。

## Suggested fix

L をどこに置くかは、リードの判断が必要です。時計は遅れの測定の基準の線 (プローブのクリックに使う `time_of_frame`) でもあるためです。

- 区間ごとに、曲の位置が耳に届く時刻に、マスターのインサートの遅れ (ブロックごとの値) を足す。たとえば区間ごとの別のずれ `master_us = master_latency / rate` を作り、`heard`、`stamp`、`segment_at` がそれを足す。一方で `frame_at` と `time_of_frame` (プローブと `vocalrec` が使うエンジンのフレーム軸) は、素の DAC 時刻のままにする。
- メトロノームのクリックを同じ L だけ遅らせる (拍のフレームから L フレーム後にクリックを始める)。こうすると、クリックと曲が再び耳で揃う。

プロトコルや曲ファイルは変更しません。`tests/lookahead.rs` のような、遅れ 100 フレームの偽のマスターのエフェクトを使うテストで確認できます。拍が聞こえる時刻に弾いた鍵が、その拍の位置で記録されることを見ます。

## Verification

コードを読んで確認しました (テストは実行していません。遅れを報告するプラグインをマスターに置く必要があり、プラグインは起動していません)。アーキテクチャ 20-8 は、マスターのインサートの遅れを 「補正はしないが、耳に届いている位置 (第 11 章) に足す」 としています。コードでは、マスターの `Strip::latency()` は `Engine::master_latency()` (`engine.rs:1646`) でだけ読まれ、それを呼ぶのは書き出し (`export/job.rs`) だけです。`dac_us` と `segment_now` は、すべての時計の区間を `playback_us` だけから作ります。メトロノームの `click_sum` は、マスターのインサートの後で出力に足されます (`engine.rs:3091-3104`)。

遅れを報告するのはプラグインのエフェクトだけです (`Effect::latency` は、組み込みのエフェクトではすべて 0 です。`effect.rs:46`)。そのため、遅れのあるプラグインをマスターに置いたときにだけ現れます。meloet 自身のリミッターでは何もずれません。第 11 章には書かれていません。`docs/progress.md` でも未解決としては挙がっていません。

Corrections: 行の記載は訂正なし。時計と遅れの測定について: プローブのクリックはマスターの後に足されます (`self.probe.add`、`engine.rs:3087`)。測定は `TransportClock::time_of_frame` を読みます (`probe.rs:199`)。歌の録音機は `frame_at` / `kept_frame_at` / `time_of_frame` を読みます (`vocalrec.rs:912, 1030`、`service/input.rs:404, 534`)。そのため、フレームの軸は素の DAC 時刻のままでなければならず、そうしないと測定される遅れがマスターの遅れの分だけ増えてしまいます。

`needs-lead` とした理由: リアルタイムの時計 (固定の語に詰めた `Segment` に新しいフィールドを足す)、メトロノームと録音位置の規則を変えるためです。また、実際に遅れのあるプラグインを使った測定が必要です。選択肢は次の 3 つです:
1. アーキテクチャ 20-8 のとおりにする。置き場所: 区間ごとの `ear_us` (`master_strip.latency() / rate`。ブロックごとに 1 回読む) を `clock.rs` の `Segment` に持たせる。時刻を曲の位置に変換する箇所 (`Segment::at`。つまり `heard_at`、`heard_now`、エンジン自身の `stamp` / `stamp_recent` / 止めた位置) だけで足し、`frame_at` / `time_of_frame` には足さない。コアの `vocal_mark` (`service/input.rs:395-412`。曲のフレームを `time_of_frame` で時刻に変える) も同じ量を足す。メトロノームのクリックは、拍のフレームから `master_latency` フレーム後に始める。かかるもの: エンジン (オーディオスレッド、時計のワードの配置)、コアの歌のマーク、`tests/lookahead.rs` のようなテスト、マスターに遅れのあるプラグインを置いた `record-timing.sh` での測定。これを推奨します。ただし、ひとつの急ぎの修正ではなく、この整理のあとに別の仕事として (meloet-very-hard で) 行うべきです。
2. メトロノームだけをマスターの遅れの分だけ遅らせる。安くすみ、クリックと曲のずれは直るが、再生の線と録音した鍵は L だけ遅れたままになる。
3. アーキテクチャ 20-8 を、足さないと書き換え、既知の制限として `docs/progress.md` に挙げる。コードは変えない。

重大度 `medium` はそのままです。録音した位置は L だけずれますが、それは遅れのあるプラグインをマスターに置いたときだけです。回避策は、録音中にそのプラグインを外すことです。

## Fix

選択肢 1 (リードの判断)。プロトコルや曲ファイル、ワイヤーの変更はありません。

- `crates/meloet-engine/src/clock.rs`: `Segment` に `ear` を持たせる。これは、そのブロックで始まる区間のマスターのインサートの遅れ (フレーム数) です。`map_gen` を入れているワードの余ったビットに入れています (リングはまだ 11 ワードあります)。`TransportClock::segment_at` は、曲がその時刻に耳に届いている最新の区間 (`start_us + ear ≤ t`) を取り、次の区間がいつ始まるかを記録します。`Segment::at` は、`t − ear` の線を読みます。遅れがより大きい後の区間がまだ耳に届いていないあいだは、区間の終わりで止まります (速度 0)。遅れがリングの届く範囲より長いときは、残っている一番古い区間から数え直します (そうしないと鍵が失われます)。新しく `TransportClock::heard_time_of_frame` を作る。これは `time_of_frame(frame + ear)` です。`frame_at`、`kept_frame_at`、`time_of_frame` は変えていません。
- `crates/meloet-engine/src/engine.rs`: `ear` は、ブロックごとに 1 回 (コマンドの後で) `master_strip.latency()` を読んで決める。その値を各区間に書き込む。メトロノームのクリックを始める 2 か所は、その遅れを与える。遅れが変わったら、待っているクリックをその差の分だけ動かす。メトロノームや `recording_only` を静かにしたら、待っているクリックは捨てる。
- `crates/meloet-engine/src/transport.rs`: `Click::start_after` (遅れ 0 なら、これまでどおりすぐ始める)。待っているクリックは固定長の配列で最大 16 個。`delay_waiting` と `drop_waiting` を作る。オーディオスレッドでメモリは取らない。
- `crates/meloet-core/src/service/input.rs`: 歌のマークは、時計の `heard_time_of_frame` を使う (処理の本体は `Worker::mark` に移し、テストから呼べるようにした。半拍の処理は変えていない)。
- `docs/architecture.md`: 12、20-8、21-3 (メトロノーム)、22-26 (マーク) を更新。

マスターに遅れのあるプラグインがないとき (`ear` = 0) は、どの数字も以前と同じです。既存のテストは変えておらず、すべて通ります。

テスト: `crates/meloet-engine/tests/lookahead.rs` で、遅れ 0、100、1000、30 000 フレームの偽のマスタープラグイン `Lag` を使う。`the_position_heard_is_behind_the_dac_by_the_masters_delay` (ループの周りでも)、`keys_played_with_the_beat_that_comes_out_land_on_it` (数え入れの規則、止めた位置、残した鍵を含む録音した鍵)、`the_metronome_clicks_with_the_song_behind_the_masters_delay` (再生中と数え入れ中、拍より長い遅れも含む)、`the_latency_measurement_does_not_hear_the_masters_delay`、`the_song_of_a_frame_is_heard_when_it_comes_out_of_the_master`、`a_change_of_the_masters_delay_never_moves_the_position_heard_back`。リングより長い 140 000 フレームのテスト `keys_land_on_the_beat_also_behind_a_delay_longer_than_the_clock_keeps`。コアでは `a_voice_is_marked_where_the_song_came_out_of_the_master` (`crates/meloet-core/src/service/input/tests.rs`。エンジンの時計で本物のディスク書き込み機を使う)。エンジンのテストのうち 5 つとコアのテストは、古いコードで失敗することを確認しました。長い遅れのテストは、古い区間に戻る処理がないと失敗することを確認しました。

テストだけで確認しています。マスターに遅れのあるプラグインを置いた実際のアプリでの測定 (`record-timing.sh`、クリックと曲) は、まだ行っていません。
