---
id: PLUG-004
title: LV2: メインスレッドがゲートを持っているあいだ、プラグインを迂回するブロックはイベントを失い、ノートオフが失われて音が鳴り続けることがある
area: PLUG
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# PLUG-004: LV2: メインスレッドがゲートを持っているあいだ、プラグインを迂回するブロックはイベントを失い、ノートオフが失われて音が鳴り続けることがある

## Where

crates/meloet-plughost/src/formats/lv2.rs:1728-1736 (`Lv2Processor::process`: `gate.enter()` が失敗すると、そのブロックは素通しされ、`block.events` は一度も見られない)
crates/meloet-plughost/src/rt.rs:178 (`process` のあとの `self.events.clear()`: 使われなかったブロックのイベントは消える)
crates/meloet-plughost/src/formats/lv2.rs:2264-2289 (`save_state` は `state.to_string` とコピーのあいだもゲートを保持する。プラグインに必要な長さより長い)
crates/meloet-plughost/src/formats/lv2.rs:2248-2262 (`set_param` は、コントロールポートの `SetParam` ごとにゲートを取る)
crates/meloet-plughost/src/formats/vst3.rs:970-980 (バスの切りかえ中や、プラグインのオン・オフ中に VST3 でも同じ素通しが起きる)

## What happens

ホストのメインスレッドが LV2 プラグインのゲートを持っているあいだ（状態を取り出すとき、状態を戻すとき、meloet からコントロールポートを設定するとき、プリセットを保存するとき）、エンジンがそのプラグインに頼むブロックは、プラグインを迂回します。音源なら、そのブロックは無音になり、エフェクトなら、ドライの入力がそのまま通ります。ブロックのイベント（MIDI のノートオンとノートオフ、オートメーションの点）は、いっしょに捨てられます。`process` は `block.events` を読む前に戻り、`SlotRun::serve` があとでそれを消すからです。

ノートオフを失うと、そのノートはプラグインのなかで鳴り続けます（エンジンはもう離れたと思っているので、曲を止めても終わりません）。ノートオンを失うと、そのノートは聞こえません。パッドやオルガンの音では、同じ鍵をもう一度弾くまで音が止まらないことがあります。

このゲートは、普通の使い方でも、曲の再生中に取られます。プラグインのあるプロジェクトを保存するたびに、各プラグインの状態を尋ねます（docs/architecture.md 20-11）。プラグインの窓に入るときと出るときにも尋ねます（20-13、15-C）。パネルの手順は `commit: true` のあとに尋ねます。コントロールポートへの `SetPluginParam` のたびにもゲートを取ります。`save_state` は、状態を Turtle の文字列として書くあいだも、ゲートを保持します。これはプラグインに触れない処理です。

## What should happen

ゲートの役目（lv2.rs:9-11）から、プラグインを動かせないブロックは音を失ってよいです。architecture 24-16 もこの穴に触れています。しかし、そのブロックのノートは、プラグインに届かなければなりません。少なくとも、離された鍵のノートオフは失われてはいけません。鍵を離したら、そのノートは終わらなければなりません。

## How to trigger

コードを読んで確かめました。実行はしていません。
1. ノートのあるトラックに、音を保ったままの Surge XT（LV2）か、ほかの LV2 シンセを置き、曲を再生します。
2. 再生中に、曲を保存します（Ctrl+S）。または、プラグインの窓に入ってから出ます。または、パラメータ欄でそのパラメータをドラッグします。
3. ゲートが保持されている時間にノートオフが入るブロックが素通しされると、そのノートは終わりの後も鳴り続けます。

## Cause

`Lv2Processor::process` は、ゲートが埋まっているとき、「そのブロックはプラグインがない」と同じに扱い、`block.events` を残さずにすぐ戻ります。次のブロックへイベントを運ぶものはありません。`save_state` は、関数の終わりまでゲートを持ちつづけるので、Turtle の書き出しでもゲートが保持され、その時間が長くなります。

## Suggested fix

`formats/lv2.rs` の小さな 2 つの変更:
1. `process` で、ゲートが埋まっているとき、そのブロックの MIDI イベントを捨てずに次のブロック用に残します（`Lv2Processor` に、読み込み時に作る固定サイズのバッファを置き、次に動くブロックの先頭フレームで MIDI ポートへ入れます。バッファがいっぱいのときは、少なくともノートオフを残します）。`Vst3Processor::process` も、まれな素通しのために同じことができます。
2. `save_state` では、`instance.state()` のすぐあとでゲートを手放します（lv2.rs:2364-2369 の `save_preset` と同じ）。`to_string` とコピーには、ゲートはいりません。

プロトコルもファイルの形も変えません。

## Verification

コードを読んで確かめました（テストは実行していません。プラグインも起動していません）。`Lv2Processor::process`（`lv2.rs:1723-1736`）は、`gate.enter()` が失敗すると `block.events` を読む前にすぐ戻ります。`SlotRun::serve`（`rt.rs:171-178`）は、何が起きても `process` のあとで `self.events` を消します。そのため、素通しされたブロックのノートオン、ノートオフ、パラメータの点は失われます。ゲートはメインスレッドで、`save_state`（`state.to_string` とコピーのあいだ）、`load_state`、コントロールポートの `set_param`、`save_preset` で取られ、どれも曲の再生中に起こりえます。`Vst3Processor::process`（`vst3.rs:970-980`）にも同じ早い戻りがあります。architecture には、ブロックが「プラグインなしで進む」（音の話）としか書かれておらず、イベントを捨ててよいとは書かれていません。重要度 `medium` は変えません（保存のときに再生中だと、ノートが鳴り続けることがあるため）。

訂正: なし。

修正案（両形式で 1 か所、新しいバッファは作らない）: `crates/meloet-plughost/src/formats/mod.rs` の `Processor::process` は、プラグインが動いたかどうかを `bool` で返す。`lv2.rs` と `vst3.rs` は、ゲートの早い戻りで `false` を返す。`crates/meloet-plughost/src/rt.rs` の `SlotRun::serve` は、ブロックが実行されたあとにだけ `self.events` を消す。残したイベントは、`len < capacity` の条件で今と同じように入れ、残したイベントのフレームは次のブロックの長さに合わせて先頭フレームに置く。また `lv2.rs` の `save_state` は、`instance.state()` のすぐあとでゲートを手放す（`save_preset` と同じ）。`to_string` とコピーは、インスタンスに触れないからです。テストは `rt.rs` のテストで、偽のプロセッサを使います。ゲートが埋まったブロックのあとに実行されるブロックが、最初のブロックのノートオフを受け取ることを確かめます。

## Fix

- `crates/meloet-plughost/src/formats/mod.rs`: `Processor::process` は、プラグインが動いたかどうかを返す。
- `crates/meloet-plughost/src/formats/lv2.rs`、`formats/vst3.rs`: ゲートの早い戻りで false（LV2 では、作られたときより長いブロックの場合も）。最後で true。VST3 にも同じ穴があったので、同じ方法で直しました。`formats/fake.rs`: true。
- `crates/meloet-plughost/src/rt.rs` の `SlotRun::serve`: プラグインが動かなかったブロックのあと、そのイベントは席のイベントバッファに残ります（読み込み時に作る。リングの大きさ + 256）。フレームは 0 に設定し、次のブロックの前に、次のブロック自身のイベントより先に送ります。バッファがいっぱいのときは、音を終わらせるイベント（ノートオフ、ベロシティ 0 のノートオン、CC 120 / 123、値が 64 未満のペダル CC 64 / 66）が、音を終わらせないイベントの代わりに入ります。`RESET` のあるブロックでは、残っている MIDI のうち音を終わらせないものを捨てます（リセットのあとに、前の鍵が始まらないようにするため）。`Handle::forget_params` は、パラメータの変更だけを捨てます。残った MIDI は残します。
- `crates/meloet-plughost/src/formats/lv2.rs` の `save_state`: ゲートは `instance.state()` と、残した値の間だけ保持します。Turtle の文字列は、ゲートを手放してから書きます（`save_preset` と同じ）。
- テスト: `rt::tests::the_events_of_a_block_the_plugin_did_not_run_go_with_the_next`（meloet-plughost。音声スレッドのコードが行うメモリ確保の数も数え、0 であることを確かめます）。
