---
id: MODEL-B-001
title: 見つからないプラグインの音源を外すと、パラメータ 0〜3 の欄が残り、サイン波のつまみを動かす
area: MODEL-B
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# MODEL-B-001: 見つからないプラグインの音源を外すと、パラメータ 0〜3 の欄が残り、サイン波のつまみを動かす

## Where

crates/meloet-model/src/mixer.rs:752-758 (`remove_missing`: `track.instrument = InstrumentRef::SINE` のあと `drop_orphan_lanes`)
crates/meloet-model/src/automation.rs:163-167 (内蔵の音で `AutoTarget::Instrument` の `Track::has_target`)
crates/meloet-model/src/knobs.rs:84-91 (`knob_usable`: サイン波は 4 つのつまみすべてを使う)

## What happens

トラックの音源であるプラグインに「見つからない物を曲から外す」を選ぶと、そのトラックはサイン波になる。そのプラグインのパラメータの欄のうち、プラグイン id が 0、1、2、3 のものは曲に残る。それらは今、サイン波のつまみの欄として読まれ (`SoundKnob::ALL[param]`: 「明るさ」、「立ち上がり」、「のび」、「ゆれ」)、プラグインの古い点が、曲の再生中にサイン波の明るさ、立ち上がり、のび、ゆれを動かす。同じプラグインの id が 4 以上の欄は消えるので、結果は、プラグインが割り当てた番号にも依存する。

## What should happen

古い音源のパラメータの欄はすべて同じ手順で消え、つまみは最初からやり直す。これは、音源を入れ替えるほかのすべての場合と同じである: docs/architecture.md 21-5 (「エフェクトを外す・入れ替える、または音源を入れ替えると、そのパラメータの欄は同じ取り消しの手で消える … モデルの `RemoveMissing` がこれを行う」) と 25-27 (「音を入れ替えると、つまみは元に戻り、その欄は同じ手で消える」)。兄弟の `edit::with_instrument` (crates/meloet-model/src/edit.rs:783-798) は、`set_instrument` と `remove_instrument` (止まったプラグインの「外す」) に使われ、新しい音源が同じプラグインでなければ、`AutoTarget::Instrument` の欄をすべて消し、`knobs` を初期化する。

## How to trigger

コードを読んで分かったこと。実行はしていない。

1. 音源がプラグインの鍵盤トラックで、そのプラグインのパラメータの 1 つの id が 0..=3 である欄を、いくつか点付きで持つ (LV2 のコントロールポートは、ポートの番号を id として使う。`crates/meloet-plughost/src/formats/lv2.rs:691`。そのため小さな id はよくある)。
2. そのプラグインがない PC で曲を開く。トラックはサイン波を鳴らし、`send_automation` は音源の欄を外す (crates/meloet-core/src/play/autosync.rs:136-143)。
3. そのプラグインに「曲から外す」を選ぶ (`MissingRef::Plugin`、crates/meloet-core/src/state.rs:2419 → `Play::remove_missing` → `mixer::remove_missing`)。
4. トラックの音源はいま `InstrumentRef::SINE`、つまり内蔵の音色になる。そのため autosync は残った欄を席へ送り、サイン波のつまみはプラグインの点に従う。欄はつまみの名前の下にも出る。

## Cause

`remove_missing` は音源の項目を直接設定し、`drop_orphan_lanes` に頼る。これは `has_target` が真になる欄をすべて残す。内蔵の音源では `has_target(Instrument { param })` は `SoundKnob::from_index(param).is_some_and(knob_usable)` であり、`knob_usable(SINE, _)` は 4 つのつまみすべてで真なので、プラグインの欄で `param` が 0..=3 のものは、対象があるものとして数えられる。また、ここでは `track.knobs` も初期化されない (`with_instrument` と違って)。

## Suggested fix

`remove_missing` で、`uses(&track.instrument)` のとき、音源を `edit::with_instrument` と同じ方法で入れ替える (`AutoTarget::Instrument` の欄をすべて消し、`knobs` を初期化する)。たとえば `with_instrument` を `pub(crate)` にして、`drop_orphan_lanes` の前に呼ぶ。プロトコルや曲ファイルの形は変えない。`lanes_go_with_what_they_move` のような、param 2 のプラグインの欄を持つテストで確かめられる。

## Verification

コードを読んで確認した (テストは実行していない)。`mixer::remove_missing` は `track.instrument = InstrumentRef::SINE` を手で設定してから `drop_orphan_lanes` を呼ぶ。内蔵の音では `Track::has_target(Instrument { param })` は `SoundKnob::from_index(param)` と `knob_usable` であり、`knob_usable(SINE, _)` は 4 つのつまみすべてで真なので、プラグインのパラメータ 0〜3 の欄は残り、つまみの欄として読まれる。`edit::with_instrument` (`set_instrument` と `remove_instrument` の経路) は `AutoTarget::Instrument` の欄をすべて消し、`knobs` を初期化する。この経路はどちらもしない。アーキテクチャ 21-5 は、欄を消す手順の 1 つとして `RemoveMissing` を挙げている。重大度 `medium` は変えない (プラグインなしで開いた曲で通常の操作をすると音が変わる。LV2 のパラメータ id はポートの番号なので、0〜3 はよくある)。

訂正: なし。

修正案: `crates/meloet-model/src/edit.rs`: `with_instrument` を `pub(crate)` にする。`crates/meloet-model/src/mixer.rs` の `remove_missing`: `uses(&track.instrument)` のとき、インサートを空にして `drop_orphan_lanes` を行う前に、`with_instrument(&track, InstrumentRef::SINE)` でトラックを作る。テスト: mixer のテストで `lanes_go_with_what_they_move` の隣に置く。param 2 と param 9 の欄を持つ見つからない音源プラグイン。`remove_missing` のあと、どちらも残らず、`knobs` は既定値になる。

## Fix

`crates/meloet-model/src/mixer.rs` の `remove_missing`: 見つからない音源で鳴っていたトラックは、`edit::with_instrument` (`crates/meloet-model/src/edit.rs` で `pub(crate)` にした) を通してサイン波になる。そのため、古い音源のパラメータの欄はすべて消え、つまみは最初からやり直す。`set_instrument` と `remove_instrument` と同じである。テスト: `automation::tests::a_missing_instrument_takes_its_lanes_and_the_knobs_along` (param 2 と 9 の欄、つまみは手で設定。手順のあとは音量の欄だけが残り、つまみは既定値になる)。
