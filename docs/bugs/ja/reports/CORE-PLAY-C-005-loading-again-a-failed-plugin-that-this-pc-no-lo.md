---
id: CORE-PLAY-C-005
title: 読み込みなおした、失敗したプラグインがこの PC にもうないと、そのトラックは sine の代わりではなく無音のままになる
area: CORE-PLAY-C
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-PLAY-C-005: 読み込みなおした、失敗したプラグインがこの PC にもうないと、そのトラックは sine の代わりではなく無音のままになる

## Where

crates/meloet-core/src/play/plugs.rs:860-873 (`reload_plugin`、`Phase::Failed`: `want_plug` の答えを見ていない)
crates/meloet-core/src/play/plugs.rs:402-405 (`known` にそのキーの束がないとき、`want_plug` は `Want::Missing` を返す)
crates/meloet-core/src/instruments.rs:127 (`InstrumentRef::Plugin(_)` → `Silent`)

## What happens

プラグイン音源が読み込めなかったトラック（`Phase::Failed`。トラックは sine 波を鳴らす）で「もう一度読みこむ」を押すと、失敗した項目を捨てて、`want_plug` にプラグインを読み込むよう頼みます。そのあと、`want_plug` の答えが何であっても `EngineOp::Instrument { instrument: Plugin(..) }` を送ります。この PC にそのプラグインがもうないとき（あとのスキャン、たとえば「もう一度スキャン」で、削除されたか移動されたプラグインが `known` から外れた場合）、`want_plug` は `Missing` を返し、何も読み込みません。しかしシートにはプラグインの代わりの無音（`Silent`）が渡されます。トラックは sine 波から無音に変わり、そのまま戻りません。画面はプラグインが見つからないと表示するので、sine 波が代わりに鳴っていることと矛盾します。

## What should happen

docs/architecture.md 20-11「見つからない物」: 「プラグインがない → 音源は sine」。`engine_instrument`（plugs.rs:343-350）は、この PC にないプラグインに sine をエンジンに渡します。同じ役目の `load_from_song`（aistate.rs:375-384）は、`want_plug(..) == Want::Loading` のときだけ楽器を送ります。`seat_instrument` は `Want::Missing` を `InstrumentRef::SINE` に対応させます（plugs.rs:446-449）。

## How to trigger

コードを読んで確かめました。実行はしていません。
1. 曲のトラックが、読み込みに失敗するプラグイン音源を使っています（`PluginUnreadable`）。そのトラックは sine 波を鳴らし、「もう一度読みこむ」を表示します。
2. プラグインを PC から外す（またはバンドルを移す）。そしてスキャンをもう一度行う（「もう一度スキャン」）。`set_known_plugins` がそのキーを落とします。
3. トラックで「もう一度読みこむ」を押す。`reload_plugin` → `drop_plug` → `want_plug` = `Missing` → `EngineOp::Instrument { Plugin }` → シートは `Silent`。

## Cause

`reload_plugin` の `Phase::Failed` の分岐では、`want_plug` が返す `Want` を捨てていて、プラグインは常にエンジンに送られます。

## Suggested fix

`load_from_song` と同じように答えを見ます。`Want::Loading` のときだけ `InstrumentRef::Plugin(plugin)` を送り、`Want::Missing` のときは `InstrumentRef::SINE` を送ります（`Kept` は、捨てたあとには起きえません）。プロトコルもファイルも変えません。

## Verification

コードを読んで確かめました（テストは実行していません）。`reload_plugin` の `Phase::Failed` の分岐（`play/plugs.rs:860-873`）では、`want_plug` の答えを捨て、`EngineOp::Instrument { instrument: InstrumentRef::Plugin(plugin) }` を常に送ります（試し弾き中でなければ）。`want_plug` は、`plugs.known` にそのキーの束がない場合（`plugs.rs:402-405`）、`Want::Missing` を返し、何も読み込みません。エンジンのプラグインの代わりは無音です。`seat_instrument`（`plugs.rs:446-449`）は `Want::Missing` を sine に対応させます。重要度 `low` は変えません（読み込みの失敗と「読みこむ」の押下のあいだに、プラグインが PC から消えている必要があるため）。

訂正: なし。

修正案: `crates/meloet-core/src/play/plugs.rs` の `reload_plugin` で、`match self.want_plug(..)` を使います。`Want::Loading` → 今まで通りプラグインの代わりを送る。`Want::Missing` → `InstrumentRef::SINE`（席はすでに sine を鳴らしているので、何も送らなくても同じです。読みやすい方を選んでください）。プラグインのテストで、失敗した項目を用意し、キーを `known` から外して読みなおし、`EngineOp::Instrument { Plugin }` が送られないことを確かめます。

## Fix

`crates/meloet-core/src/play/plugs.rs` の `reload_plugin`、`Phase::Failed`: プラグインの代わりの無音は、`want_plug` が `Want::Loading` を返したときだけエンジンへ送ります（`load_from_song` と同じ）。`Want::Missing` のときは何も送らず、席は今鳴らしている sine を保ちます。`instruments.rs` は変えません。

テスト: `play::plugs::tests::loading_again_a_plugin_this_pc_no_longer_has_leaves_the_sine_wave`。
