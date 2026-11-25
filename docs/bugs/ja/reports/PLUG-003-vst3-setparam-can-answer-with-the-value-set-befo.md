---
id: PLUG-003
title: VST3: SetParam が、設定される前の値で答えることがある。param() が、音声スレッドの古い値を新しい値の上に再生するため
area: PLUG
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# PLUG-003: VST3: SetParam が、設定される前の値で答えることがある。param() が、音声スレッドの古い値を新しい値の上に再生するため

## Where

crates/meloet-plughost/src/control.rs:809-824 (`Request::SetParam`: `controller.set_param`、次に `audio.set_param`、答えのために `c.param(id)`)
crates/meloet-plughost/src/formats/vst3.rs:1410-1414 (`Vst3Controller::param` は、値を設定したあとに `hear_audio()` を呼ぶ)
crates/meloet-plughost/src/formats/vst3.rs:1167-1176 (`hear_audio` は、キューにある値を古いものから順に、すべて `setParamNormalized` で適用する)
crates/meloet-plughost/src/formats/vst3.rs:987-997 (ブロックごとに、各 `Param` イベントの最後の値を `to_controller` へ送る。meloet が設定した値も含む)

## What happens

VST3 プラグインに `SetParam {value: v2}` が来ると、コントローラーに v2 を設定し、そのあと `param()` で答えを読みます。`param()` は最初に `hear_audio()` を実行します。これは音声スレッドからのリングを空にし、なかの各値を `setParamNormalized` で適用します。メインスレッドがリングを最後に空にしてから、同じパラメータの前の `SetParam {value: v1}` を、あるブロックがすでに処理していた場合、リングには `(id, v1)` がまだ残っています。それが v2 の上に適用されます。すると答えは v1（値と文字）になり、プラグインのコントローラー（開いていれば、その窓も）は、次のブロックが v2 の反映を聞くまで（ブロック 1 つ分、最大 10 ms 後まで）v1 を表示します。

答えは、パラメータ欄の行（crates/meloet-core/src/play/params.rs の `param_now`。ドラッグしている行の値と文字）と、AI の設定にも届きます。欄でドラッグしているあいだ、行の文字が一段ずつ戻ることがあります。マウスを離す直前の動きのあとに離すと、`commit: true` の答えが前の値になることがあり、そのあと行を読み直すものはないため、欄はプラグインと一段ずれた値を表示し続けます。

## What should happen

docs/architecture.md 20-12: 「`false` でも `paramValues` に値と文字を返す（プラグインがドラッグしている行の文字を書く）」。crates/meloet-plugin/src/wire.rs:89-90: `SetParam` は「`Param` で答える。プラグインが受け取った値を、その文字と一緒に」。いま設定した値が一番新しいものです。音声スレッドから届いた、それより前の値を、その上に載せてはいけません。LV2 にはこの問題がありません。`Lv2Controller::param` は `Shared` に残した値を読み、何も再生しません（lv2.rs:2233-2236）。

## How to trigger

コードを読んで確かめました。実行はしていません。音声ブロックがホストのメインスレッドの 10 ms の刻み（`TICK`、control.rs:33）より長い必要があります。そのため、512 フレーム以上の量子で現れます（1024 フレーム / 48 kHz のドラッグの手順の約 30 % で。256 ではまれです）。
1. VST3 のエフェクトか音源（ZamComp、Surge XT）を読み込み、そのパラメータがパラメータ欄に出るように選びます。
2. 連続するパラメータを欄でドラッグします。画面は約 16.7 ms ごとに `SetPluginParam {commit: false}` を送り、離すと `commit: true` を送ります。
3. v1 を受け取るブロックが、メインループの最後の `poll` の後に終わり、`SetParam {v2}` が届く前に終わると、v2 への答えが v1 を運びます。

## Cause

`hear_audio` は、音声スレッドがプロセッサに渡した値（オートメーション）にコントローラーを合わせるためのものです。値がキューに入ったあとで、メインスレッドが同じパラメータに新しい値を設定したことは知りません。そして `param()` は、`set_param` のあとに、それを呼びます。リングには、meloet 自身の `SetParam` の値のエコーも入っています（ブロックの `told` のループは、すべての `ToPlugin::Param` を受け取るからです）。

## Suggested fix

`SetParam` は、値を設定したあとでリングを再生せずに、コントローラーから答えます。`Vst3Controller::set_param` が `setParamNormalized` の前に `self.hear_audio()` を呼ぶようにし、答えは `hear_audio()` を呼ばない形の `param()` で読みます（フラグか、`SetParam` の分岐だけが使う小さな別メソッド）。その時点で動いていたブロックからの v1 のエコーは、あとでコントローラーを一時的に通ることがありますが、答えとパラメータ欄の行は正しくなり、v2 のエコーが続きます。プロトコルもファイルの形も変えません。

## Verification

コードを読んで確かめました（テストは実行していません）。`Request::SetParam`（`control.rs:804-824`）は `controller.set_param(id, v2)`（`vst3.rs:1459`、ただの `setParamNormalized`）を呼び、そのあと答えに `c.param(id)` を使います。`Vst3Controller::param`（`vst3.rs:1410-1414`）は最初に `hear_audio()` を実行し、それは音声スレッドがキューに入れた `(id, value)` をすべて適用します（`vst3.rs:987-997` が、ブロックごとの `ToPlugin::Param` の最後の値を、meloet 自身のものも含めてキューに入れます）。前に設定された値のエコーがまだリングにあると、それが v2 の上に載り、答えとして読まれます。LV2 の `param` は、残した値を読むだけで、何も再生しません。重要度 `low` は変えません（ホストの 10 ms の poll と次の `SetParam` のあいだにブロックが終わる必要があり、次に読むまで行が一段ずれたままになるため）。

訂正: なし。

修正案: `crates/meloet-plughost/src/formats/vst3.rs`: `Vst3Controller::set_param` が `self.hear_audio()` を呼んでから `setParamNormalized` で値を設定し、`SetParam` の分岐は、設定のあとでリングを再生しない答えを返します（`hear_audio` を使わない非公開の `param_now(id)` を使い、`param()` は自分の `hear_audio()` のあとに使う）。`crates/meloet-plughost/src/control.rs` は、トレイトに新しいメソッドが要るときだけ、トレイトを通してそれを使います。そうでなければ、変更は `vst3.rs` の中に収め、次の `param` の呼び出しのために「just set」を覚えておきます。テストは、`control.rs` のテストに偽の VST3 風コントローラーがあればそれで、なければ `vst3.rs` の順番の単体テストで行います。

## Fix

- `crates/meloet-plughost/src/formats/vst3.rs`: `Vst3Controller::set_param` は、最初にリングを聞く（前の値が新しい値より先に入る）。値を設定し、その id を `just_set` として残す。次の `hear_audio`（`SetParam` に答える `param()` の中のもの）は、設定より前に与えられた、そのパラメータのリングの値を飛ばす（`Handle::set_param` が席を待っているあいだに、前の値で動いたブロック）。ほかのパラメータは今まで通り聞く。トレイトと `control.rs` は変えない。
- テスト: `formats::vst3::tests::a_value_set_is_answered_over_the_one_before_it_heard_from_the_audio_thread`（`#[ignore]`、Surge XT（VST3）が必要）。書いたが、この修正では実行していない（依頼が、ignore のテストを実行しないよう求めていたため）。実際のプラグインがなければ試せる偽の VST3 コントローラーはない。
