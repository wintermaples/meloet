---
id: CORE-PLAY-C-001
title: サンプルレートを変えたあとに、止まったプラグインを読みなおすと古いサンプルレートで動く
area: CORE-PLAY-C
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-PLAY-C-001: サンプルレートを変えたあとに、止まったプラグインを読みなおすと古いサンプルレートで動く

## Where

crates/meloet-core/src/play/plugs.rs:668 (`load_again` は `Phase::Stopped | Phase::Reloading` を飛ばす)
crates/meloet-core/src/play/plugs.rs:855-858 (`reload_plugin`: `Effect::ReloadPlugin { ticket }` にはレートが入っていない)
crates/meloet-core/src/play/plugs/aistate.rs:362-365 (`reload_at`、同じ効果)
crates/meloet-core/src/plugins/hosts.rs:1165-1172 (`Run::reload` は `slot.spec`、つまり最初の読みこみの仕様で読む)
crates/meloet-core/src/plugins/hosts.rs:1110-1117 (`Request::Load { sample_rate: spec.sample_rate, .. }`)
crates/meloet-core/src/service.rs:3364-3373 (`Effect::ReloadPlugin` → `Hosts::reload(id, block, ..)`)

## What happens

プロセスが止まった(または `Phase::Reloading` で読みなおし中の)プラグインは、出力が一つのサンプルレートで動いている間は、「もう一度読みこむ」(または AI リンクの復活)を、出力が別のレートで開かれたあとに使うと、古いレートで読みこまれます。プラグインのホストには `Request::Load` で古いレートが伝えられますが、エンジンは新しいレートで動いています。そのため、音源は 48 kHz と伝えられた状態で 44.1 kHz のエンジンの上で動き、約 1.5 半音低く、遅く鳴ります。テンポに合わせた LFO やディレイも狂います。エフェクトでは、ディレイ、フィルター、リバーブの時間が狂います。使う人には何も知らされず、プラグインは準備完了と表示されます。

## What should happen

どのプラグインも、ストリームのレートで動くべきです。`Play::set_plug_rate` には「プラグインは一つのレート向けに作られているので、レートが前と違うなら、すべて読みなおす」と書かれており(plugs.rs:692-699)、`want_plug` と `load_again` は必ず `sample_rate: self.plugs.rate` を送ります。docs/architecture.md 20-10 には「読みなおし: 新しいプロセスで、最後に分かった状態と、そのあとに動かしたパラメーターを戻す」とあります。このとき使うレートは、今エンジンが動いているレートです。一度も読みこまれなかったプラグイン(`reload_plugin` の `Phase::Failed`)の経路は正しく、`want_plug` を通るので今のレートを使います。

## How to trigger

コードを読んで確認したもので、実行はしていません。
1. Surge XT のようなプラグイン音源を使う曲を、48 kHz の出力で再生する。
2. プラグインのプロセスが止まる(クラッシュ、または 5 秒後に強制終了されたハング)。トラックには「止まりました」と「もう一度読みこむ」が表示される。
3. 44.1 kHz で動く別の出力を選ぶ(`Input::AudioOpened { sample_rate: 44_100 }` → `set_plug_rate` → `load_again(true)`。止まったプラグインは飛ばされる)。
4. 「もう一度読みこむ」を押す: `Effect::ReloadPlugin` → `Hosts::reload` → `place(&slot.spec, ..)` が `sample_rate: 48_000` で読みこむ。再生すると、トラックは約 1.5 半音低く鳴る。
手順 3 の時点で `Phase::Reloading` だったプラグインでも同じことが起きます(すでに頼んだ読みなおしが 48 kHz で答えるため)。

## Cause

`Hosts` は最初の読みこみの `LoadSpec` を `Slot::spec` に保持し、`Run::reload` はそれを変えずに読みこみます。仕様を更新するのは `SetOutputs` だけです(hosts.rs:752-759)。コアの `load_again` は `Ready` と `Loading` のプラグインを新しいレートで読みなおしますが、`Stopped` と `Reloading` では `continue` して飛ばします。あとから届く `Effect::ReloadPlugin { ticket }` にはレートがないため、レートが変わったことが `Hosts` に伝わりません。

## Suggested fix

読みなおしに今のレートを渡します。`Effect::ReloadPlugin { ticket, sample_rate: self.plugs.rate }` と `Hosts::reload(id, sample_rate, block, done)` にし、`place` の前に `slot.spec.sample_rate = sample_rate` を設定します。`Phase::Reloading` のプラグインでレートが変わったときは、もう一度読みなおすよう頼む(または `plugin_loaded` で一度だけ読みなおすよう印をつける)ことができます。プロトコル、wire、ファイルは変えません。CORE-PLAY-C-006 も参照してください(`load_again` の同じ `continue` による、別の不具合です)。

## Verification

コードを読んで確認しました(テストは実行していません)。`load_again`(`play/plugs.rs:668`)は `Phase::Stopped | Phase::Reloading | Phase::Failed` を飛ばします。止まったプラグインの「もう一度読みこむ」は `Effect::ReloadPlugin { ticket }` を送り(`plugs.rs:855-858`、`aistate.rs:365` も同じ)、サービスは `Hosts::reload(id, block, ..)` を呼び(`service.rs:3364-3373`)、`Run::reload`(`hosts.rs:1165-1172`)は `slot.spec` で読みこみます。その `sample_rate` は最初の読みこみのもの(`Request::Load { sample_rate: spec.sample_rate }`、`hosts.rs:1110-1117`)です。新しいレートを `Hosts` に伝える経路はありません。`Failed` のプラグインは `want_plug` を通り、今のレートを受け取ります。

重大度を `medium` から `low` に変えました。プロセスが止まったプラグインがあり、読みなおす前に別のサンプルレートの出力が使われる、という条件が必要だからです。そのときに音が狂うのは確かですが、まれな場合です。回避策は、プラグインを外してから入れなおすことです。

訂正: なし。

修正案: `crates/meloet-core/src/state.rs` / `play/plugs.rs` / `play/plugs/aistate.rs`: `Effect::ReloadPlugin { ticket, sample_rate: self.plugs.rate }`。`crates/meloet-core/src/service.rs` がそれを渡す。`crates/meloet-core/src/plugins/hosts.rs` の `reload(id, sample_rate, block, done)` は、`place` の前に `slot.spec.sample_rate` を書き込む。`Phase::Reloading` のときにレートが変わった場合: プラグインに、レートが変わったことを覚えさせ、`plugin_loaded` で通常の経路を通して一度だけ読みなおす(そのとき `Ready` なので `load_again` の規則が当てはまる)。テストは `state/added_tests.rs` の `plugins_are_loaded_again_for_another_sample_rate_and_for_a_new_engine` の隣(読みなおしの効果が新しいレートを持つこと)と、偽のホストを使う hosts のテスト(読みなおしの `Load` が新しいレートを名指しすること)に置きます。

## Fix

`Effect::ReloadPlugin` が `sample_rate` を持つようになりました(`crates/meloet-core/src/state.rs`)。`reload_plugin`(`play/plugs.rs`)と `reload_at`(`play/plugs/aistate.rs`)は今のレートを渡し、サービス(`service.rs`)がそれを送ります。`Hosts::reload(id, sample_rate, block, done)` は、新しいプロセスがプラグインを読みこむ前に、スロットの仕様へ書き込みます(`plugins/hosts.rs`)。plug は頼んだときのレートを覚えます(`Plug::rate`)。読みなおし中にレートが変わったものは、状態があればその状態を持って、もう一度読みなおします(`plugin_loaded` → `load_again_at`)。`tests/plugin_hosts.rs` と `tests/plugin_windows.rs` で `Hosts::reload` を呼ぶ箇所はレートを指定します。

テスト: `play::plugs::tests::a_stopped_plugin_is_loaded_again_at_the_rate_the_stream_runs_at_now`(読みなおしの効果が新しいレートを持つこと。別のレートに追い越された読みなおしは、そのレートで読みなおされること)と、`tests/plugin_state_put.rs` の `a_reload_is_at_the_rate_asked_for_and_without_a_state_tried_and_never_read_back`(読みなおしの `Load` が新しいレートを名指しすること。偽のプラグインホストを使用)。
