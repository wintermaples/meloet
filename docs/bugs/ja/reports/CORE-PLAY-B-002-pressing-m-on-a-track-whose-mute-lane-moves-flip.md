---
id: CORE-PLAY-B-002
title: ミュート・レーンが動いているトラックで M を押すと、聞こえている状態ではなく、見えていない手のミュートが反転する
area: CORE-PLAY-B
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-PLAY-B-002: ミュート・レーンが動いているトラックで M を押すと、聞こえている状態ではなく、見えていない手のミュートが反転する

## Where

crates/meloet-core/src/play/mix.rs:20-23 (`toggle_mute`: `edit::toggle_mute` が `t.muted` を反転させ、その後 `rest_with`)
crates/meloet-core/src/play/automation.rs:916-917 (同じ仕組みの `mute_pressed` は、レーンの値の反対を書く)

## What happens

トラックにミュートのレーンがあり、それが動いている（休みではなく、点がある）とします。停止中、または「● 書く」がオフの再生中に、使う人が M を押します。聞こえている状態と、ミキサーの M が示す状態は、再生線でのレーンの値です（`ui/src/components/mixer/MixerStrips.svelte:99-103`、`shownMuted`）。曲の手で決めた `muted` ではありません。使う人には見えません。M を押すと、その見えていない手の値が反転し、同時にレーンを休みにします。再生線でレーンが与える値と、手の値がすでに同じだった場合、押しても聞こえる状態も表示も変わりません。ただし、レーンは「休み」になります。

- 再生線でレーンが「ミュート」(1.0) を示し、手のミュートはオフの場合：M は点灯し、トラックは鳴りません。ミュートを解除するために M を押すと、手のミュートがオンになり、レーンが休みになります。トラックは鳴らないままで、M も点灯したままです。もう一度押す必要があります。
- レーンが「鳴っている」(0.0) を示し、手のミュートがオンの場合（レーンはトラックをミュートした後で描かれた）：ミュートにするために M を押すと、手のミュートがオフになります。トラックは鳴り続けます。

状態行には、ミュートが手で動かされ、そのオートメーションが休みになったと表示されます（`StatusMessage::AutomationRested`、`msg.automationRested`）。しかし、何も変わっていません。

## What should happen

仕様 3（Y16、`docs/spec/3-bands.md:204`）: オートメーションのあるパラメーターを手で動かすと、レーンは休みになり、「値は手が置いたところに残る」。ミュートのボタンでは、手が置くのは、表示している値の反対です。同じ機能の隣の経路は正しく動いています。「● 書く」がオンで再生中の場合、`mute_pressed`（`automation.rs:916-917`）は `lane.value_at(tick)` を読み、その反対を書きます。`docs/spec/7-readings.md:445`（ミュートの組）も、ミュートが変わったときだけ、「M を押すのと同じように」レーンを休みにすると書いています。

テスト `a_mute_lane_takes_the_place_of_the_mute_put_by_hand`（`play/automation/tests.rs:229-233`）は、今の結果を確認しています（手のミュートがオン、レーンは 0.0、押すと手のミュートはオフ、`silent` は変わらない）。修正すると、このテストも変える必要があります。

## How to trigger

コードを読んで確認したもので、実際には動かしていません。
1. トラック 1 にミュートのレーンを足し、tick 0 に 1.0 の点を 1 つ置きます（トラックは鳴らなくなり、ミキサーの M が点灯します）。手のミュートはオフのままです。
2. 停止中（または「● 書く」がオフの再生中）に、ミキサーでトラック 1 の M を押します。
3. トラックは鳴らないままで、M も点灯したままです。レーンは休みになっています。取り消しの数は +1 です。

## Cause

`toggle_mute` は `edit::toggle_mute(self.doc(), track)` で編集を作ります。これは `Track::muted` を反転させます。この値は手で決めた値で、レーンが何を鳴らしているかは関係ありません。その後 `rest_with` がレーンを休みにするので、以後は手の値が効きます。手の値がレーンの再生線での値と同じだった場合、新しい手の値は、使う人が頼んだものの反対になります。

## Suggested fix

`toggle_mute` で、ミュートのレーンが動いている場合、目標の値を `mute_pressed` と同じように決めます: `wanted = !(lane.value_at(self.play_line()).unwrap_or(hand) >= 0.5)`。そして、`muted = wanted`（反転ではなく設定）を、`rest_with` による休みとあわせて書きます（手の値がすでに `wanted` なら、編集は変わらず、`apply_resting` が休みの手だけを作ります）。ミュートの値を設定する小さなモデルの関数が必要です（または `t.muted != wanted` のときだけ反転させる）。プロトコルやファイルの変更はありません。上のテストは変わります。

## Verification

コードを読んで確認しました。既存のテスト `a_mute_lane_takes_the_place_of_the_mute_put_by_hand`（`play/automation/tests.rs:193-237`）が、今の動きそのものを示しています。手のミュートがオン、0.0 の点があるミュートのレーン（トラックは鳴っていて、M は点灯していない）、`ToggleMute` → レーンが休みになり、手のミュートはオフになり、`silent` は空です。使う人がミュートにするために M を押した後も、トラックは鳴り続けます。

意図したものかどうかは分かりません。仕様 3（Y16、`3-bands.md:204`）では、レーンは休みになり、「値は手が置いたところに残る」とあります。ボタンでは、手が置くのは表示している値の反対です。ミキサーが示すのは、再生線でのレーンの値（`MixerStrips.svelte:99-103` の `shownMuted`）で、手の中の `muted` ではありません。`7-readings.md` には、ミュートのレーンの下での M についての読み方がありません（445 行目は、ミュートの組がレーンを「M を押すのと同じように」休みにすると言うだけです）。アーキテクチャ 21-11 は、レーンが動く操作について「値と休みを一度の手で」と書いています。`toggle_mute` のコメントは「ミュートとその休みを一つの手として」と書いています。どちらも、値が手の中の値の反転だとは書いていません。同じ押し方で「● 書く」がオンの場合の隣の `mute_pressed`（`play/automation.rs:904-922`）は、`lane.value_at(tick)` を読み、反対を書きます。そのため、このテストは最初の作りの偶然を固定しているだけで、決めたことを示すものではありません。修正すると変わります。重大度 `medium` のままです。

修正の計画: `crates/meloet-core/src/play/mix.rs` の `toggle_mute` で、トラックのミュートのレーンが動いている場合、`wanted = !(lane.value_at(self.play_line()).unwrap_or(hand) >= 0.5)` とします。編集は `t.muted != wanted` のときだけ作り（`edit::toggle_mute`）、それ以外は `Err(EditError::Unchanged)` を `rest_with` に渡します。すると、rest_with が休みの手だけを作ります。モデルの変更は不要です。テスト: `a_mute_lane_takes_the_place_of_the_mute_put_by_hand` の最後の部分を変えます（押した後、トラックは鳴らず、`muted` は true）。もう一つの場合を足します（レーンが 1.0、手はオフ：押すとトラックは鳴り続け、`muted` は false）。リードが Y16 を別の意味に読むなら、コードを残して読み方を `7-readings.md` に書くことになります。その案は勧めません。

## Fix

`crates/meloet-core/src/play/mix.rs` の `toggle_mute`（画面の M）は、ミュートのレーンが動いている場合、手のミュートを、再生線 `lane.value_at(self.play_line())` でレーンが与える値の反対に設定し、レーンを休みにします。手のミュートがすでにその値なら、手の手は休み（`RestLane`）だけです。AI の `set_mute` は、`get_mixer` が示す手のミュートを設定するもので、`ToggleMute` を送ることで手のミュートを反転させ続けていました。`crates/meloet-core/src/state/ai.rs` は、AI の `ToggleMute` を新しい `Play::ai_toggle_mute` に送るようにしました。`docs/architecture.md` の 21-11 は両方を書いています。テスト: `play::automation::tests::a_mute_lane_takes_the_place_of_the_mute_put_by_hand` を変更（手のミュートがオン、レーンは 0.0: 押すと手のミュートはオンのまま、レーンは休み、トラックは鳴らない。以前は手のミュートがオフになり、トラックは鳴り続けた）。`play::automation::tests::the_mute_writes_a_point_or_rests` を変更（レーンが 1.0 を与える停止中に押すと、手のミュートはオフのままで、手は `RestLane`。以前は手のミュートがオンになった）。新しく `play::automation::tests::m_on_a_mute_lane_that_mutes_lets_the_track_play` と `play::mix::tests::the_ai_flips_the_mute_put_by_hand_whatever_its_lane_gives` を追加。
