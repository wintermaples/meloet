---
id: MODEL-B-004
title: 歌のトラックが、meloet の内蔵音の音つまみのレーンを受け入れ、何も動かさない
area: MODEL-B
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# MODEL-B-004: 歌のトラックが、meloet の内蔵音の音つまみのレーンを受け入れ、何も動かさない

## Where

crates/meloet-model/src/automation.rs:163-167 (`Track::has_target`、トラックの音源がプラグインでない `AutoTarget::Instrument`)
crates/meloet-model/src/knobs.rs:84-91 (`knob_usable` はトラックの種類を見ない)

## What happens

`add_lane(doc, vocal_track, AutoTarget::Instrument { param: 0..=3 }, …)` が成功します。歌のトラックに「明るさ」「立ち上がり」「のび」「ゆれ」のレーンができ、取り消しの手「帯を足す」とアレンジのレーンの行もできます。その点は、歌のトラックの座席に音つまみの値として送られますが、歌のトラックには音つまみを使う音がないので、聞こえるものは何も変わりません。`Doc::check_lanes` もそのようなレーンを受け入れます（曲のファイルからの場合も）。AI の接続もこれに届きます。`add_lane` で、対象が `instrument` で `param` が 0..=3 の歌のトラックは、拒否されません（crates/meloet-ai/src/tools/automation.rs:137-143 → `Command::AddLane` → `Play::add_lane`、crates/meloet-core/src/play/automation.rs:508-521。AI の試し実行、crates/meloet-core/src/play/ai.rs:420-430 も「できる」と答えます）。そのため、AI は成功したと伝えられます。

## What should happen

`has_target` のコメント: トラックは `target` が動かすものを「持っている」—— プラグインを音源として持つ、または meloet の内蔵音が使う音つまみを持つ。歌のトラックには音源がありません（crates/meloet-model/src/audio.rs:14-15「one の `Track::instrument` はサイン波で、それを読むものはない」）。音つまみは、キーのトラックの内蔵音に属します（docs/architecture.md 25-27。`Play::set_sound_knob` は、音が名前のものであるトラックにだけ働き、画面は音つまみをキーのトラックにだけ出します。crates/meloet-core/src/play/mixer.rs:598）。そのため、歌のトラックでは `has_target(Instrument { .. })` は false であるべきで、AI には「そのようなレーンはない」（`NoSuchInsert` に対する `AiRefusal::NoSuchLane`、crates/meloet-core/src/play/ai.rs:451）が返るべきです。

## How to trigger

コードを読んで確認したもので、実際には動かしていません。AI の接続をオンにして、歌のトラックを足し、ツール `add_lane` を `{"track": <the vocal track>, "target": "instrument", "param": 0}` で呼びます。曲には、歌のトラックに「明るさ」のレーンができます。その上の `put_points` も動きますが、音は何も変わりません。

## Cause

`AutoTarget::Instrument` では、`has_target` は `knob_usable(&self.instrument, knob)` だけを見ます。歌のトラックの `instrument` は `InstrumentRef::SINE`（内蔵音）で、`knob_usable(SINE, _)` はすべての音つまみで true になるので、トラックの種類は見られません。

## Suggested fix

`Track::has_target` で、`AutoTarget::Instrument { .. }` のとき、`self.kind.is_vocal()` なら false を返します（同じ理由で、`knobs::set_sound_knob` も歌のトラックを拒否させます）。曲のファイルの読み込みは、すべてのレーンについて `has_target` を聞き、対象のないレーンを外します（crates/meloet-core/src/songfile.rs:684）。そのため、そのようなレーンを持つファイルも、レーンを外して開けます。曲のファイルの形式やプロトコルの変更はありません。

## Verification

コードを読んで確認しました（テストは実行していません）。`Track::has_target(Instrument { param })` は `self.instrument` だけを見ます。歌のトラックの音源は `InstrumentRef::SINE`（「それを読むものはない」）なので、パラメーター 0 から 3 は通ります。`auto::add_lane`（`automation.rs:323`）と AI の試し実行（`play/ai.rs:422-430`）は、どちらも `has_lane_target` だけを聞くので、歌のトラックに `instrument` を指定した AI の `add_lane` はレーンを作り、成功したと伝えます。画面は歌のトラックに音つまみのレーンを出さないので、使う人がそこに届くのは AI の接続か曲のファイルを通してだけです。重大度 `low` のままです。

訂正はありません。

修正の計画: `crates/meloet-model/src/automation.rs` の `Track::has_target`: `self.kind.is_vocal()` のときは `AutoTarget::Instrument { .. }` が false。すると、`add_lane` は `NoSuchInsert` を返し（AI の add_lane には `NoSuchLane`）、check_lanes はそのようなレーンを拒否し、曲のファイルの読み込みはそれを外すので、そのレーンを持つファイルも、レーンを外して開けます。テスト: 自動化のテスト（モデル）で、歌のトラックへの add_lane が拒否されること。`knobs::set_sound_knob` に歌のトラックから届くかどうかは、修正中に確認します。届くなら変えます。

## Fix

`crates/meloet-model/src/automation.rs` の `Track::has_target`: 歌のトラックでは `AutoTarget::Instrument` が false になります。すると、`add_lane` は `NoSuchInsert` を返し（AI の `add_lane` は `NoSuchLane` で答えられます）、`check_lanes` はそのようなレーンを拒否し、曲のファイルの読み込みはそれを外します。そのため、そのレーンを持つファイルも、レーンを外して開けます。`knobs::set_sound_knob` は変えていません。画面は歌のトラックに音つまみを出さず（`play/mixer.rs` の `picked_at`）、AI のツールも音つまみを設定しないからです。テスト: `automation::tests::a_vocal_track_has_no_lanes_of_an_instrument`（モデル）と `songfile::vocal_tests::a_lane_of_an_instrument_on_a_vocal_track_is_left_out`（コア: ファイルは開き、そのレーンは外され、音量のレーンは残る）。
