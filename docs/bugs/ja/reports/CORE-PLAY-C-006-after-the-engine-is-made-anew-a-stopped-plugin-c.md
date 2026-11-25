---
id: CORE-PLAY-C-006
title: エンジンを作り直した後、止まったプラグインを読みこみ直せなくなる
area: CORE-PLAY-C
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-PLAY-C-006: エンジンを作り直した後、止まったプラグインを読みこみ直せなくなる

## Where

crates/meloet-core/src/play/plugs.rs:701-707 (`plugs_lost` → `load_again(false)`)
crates/meloet-core/src/play/plugs.rs:668 (`Phase::Stopped | Phase::Reloading | Phase::Failed => continue`)
crates/meloet-core/src/service.rs:2587-2598 (`unload_gone`: 失われたエンジンと一緒に返された止まったスロットは、`ids` から取り除かれ、`Hosts` で読み込みを解除される)
crates/meloet-core/src/service.rs:3364-3373 (`Effect::ReloadPlugin` で `ids[ticket]` がないと、`Input::PluginUnreadable` になる)
crates/meloet-core/src/play/plugs.rs:603-604 (`Phase::Reloading` → `Phase::Stopped` に戻る)

## What happens

エンジンがストリームと一緒に失われ、作り直されると (`service.rs` の `Audio::open` → `rebuild` → `Input::PluginsLost`)、音を出しているプラグインはすべて新しい ticket で読みこみ直される。しかし、止まっていたプラグインは飛ばされる。失われたエンジンのスロットは捨てられ、それぞれの `Gone` によって `unload_gone` が、止まったプラグインの ticket を `PluginSide::ids` から取り除き、`Hosts` でそのスロットの読み込みを解除する (再読みこみのために `Hosts` が覚えていた状態とパラメータも一緒に)。コアはそのプラグインを止まったものとして載せ続け、画面は「もう一度読みこむ」を出し続けるが、それを押しても何も起きない。`ReloadPlugin` は id を見つけられず、`PluginUnreadable` を返し、プラグインは `Stopped` に戻る。音を出す楽器は黙ったまま、エフェクトは通り抜けたまま。人がそれを削除するか、曲を開き直すまで続く。AI の link の復活 (`reload_at`) も同じ経路で失敗する。

## What should happen

docs/architecture.md 20-10: 「読みこみ直し: 新しいプロセスで、最後に分かった状態と、そのあと動かしたパラメータを戻す」。`stopped_plugins` と `plugin_is_down` は、そのようなプラグインに対して「もう一度読みこむ」を出し続けるので、それは動くべきである (または、そのプラグインは曲の内容から他のものと一緒に読みこみ直されるべきである)。

## How to trigger

実行せず、コードを読んで確かめた。
1. 曲のプラグインが止まる (プロセスがクラッシュする)。「…が止まりました」と「もう一度読みこむ」が出る。
2. 出力がストリームと一緒にエンジンを失わせる形で失われ (cpal の失敗の経路。`recover` が何も返さない)、出力がもう一度開く: `rebuild`、`PluginsLost`。
3. 「もう一度読みこむ」を押す: `Effect::ReloadPlugin` → `ids[ticket]` がない → `PluginUnreadable` → まだ止まったまま。

## Cause

`load_again` は止まったプラグインを読みこまない。再読みこみの経路は、古いスロットがまだ `PluginSide::ids` と `Hosts` に知られていることに頼っている。失われたエンジンを捨てると、それらが取り除かれてしまう。`ReloadPlugin` には、曲からの読みこみへ戻る手段がない。

## Suggested fix

`load_again(false)` で、`Stopped` のプラグインを他と同じように新しく読みこむ (`PlugState::Bytes(old.plugin.state)` または `Fresh`、新しい ticket、`plugs.stopped` から除く)。または、サービスで、ticket に id がない `ReloadPlugin` を、曲からの読みこみを試してからだけ `Input::PluginUnreadable` にする (ルールは `plugin_unreadable` で `Phase::Reloading` のとき `load_from_song(spot)` をできる)。関連するが別の不具合として CORE-PLAY-C-001 (同じ `continue` が、後の再読みこみに古いサンプルレートを残す)。プロトコルとファイルの変更はなし。

## Verification

テストを走らせず、読んで確かめた。`plugs_lost` (`play/plugs.rs:701-707`) は `load_again(false)` を呼ぶ。これは `Phase::Stopped` を飛ばす (`plugs.rs:668`)。ストリームと一緒に失われたエンジンは、止まったプラグインのスロット (`HousedInstrument` / `HousedEffect`、`plugins/housed.rs`) を持っていた。それが捨てられると、その `Gone` がサービスに伝わり、`unload_gone` (`service.rs:2587-2598`) が ticket を `ids` から取り除き、`Hosts` でスロットの読み込みを解除する。その後の `Effect::ReloadPlugin` は id を見つけられず (`service.rs:3364-3373`)、`Input::PluginUnreadable` を返す。これは `Reloading` を `Stopped` に戻す (`plugs.rs:603-604`)。「もう一度読みこむ」は出たままで、何もしない。重大度 `low` は変えない (止まったプラグインと、ストリームと一緒に失われたエンジンが必要。コード自身も、これを失敗の経路として扱っている: 「前のストリームで失われたエンジンなので、新しく作る」)。

訂正: なし。

修正案: `crates/meloet-core/src/play/plugs.rs` の `load_again`: エンジンが作り直されたとき (`live` が false のとき) は、`Stopped` のプラグインを他と同じように、曲の状態 (`PlugState::Bytes(old.plugin.state)` または `Fresh`) から、新しい ticket で読みこむ。古い ticket は `plugs.stopped` から除く。`Reloading` と `Failed` はそのまま。最後の状態のあとに `Hosts` が覚えていたパラメータ変更は、この隅では失われる。読みこめないプラグインに対しては、これは受け入れられる。テストは `plugins_are_loaded_again_for_another_sample_rate_and_for_a_new_engine` の隣に置く。

## Fix

`crates/meloet-core/src/play/plugs.rs` の `load_again_at` (`load_again` の本体): 新しいエンジンのとき (`live` が false)、`Phase::Stopped` のプラグインは、他と同じように、曲の状態 (`Bytes`、空なら `Fresh`) で新しい ticket をもらって読みこまれる。古い ticket は `plugs.stopped` から出る。別のサンプルレート (`live`) のときは、今と同じく「もう一度読みこむ」を待つ。`Reloading` と `Failed` は前のままにする。

テスト: `play::plugs::tests::a_stopped_plugin_is_loaded_anew_as_the_song_has_it_when_the_engine_is_made_anew`。
