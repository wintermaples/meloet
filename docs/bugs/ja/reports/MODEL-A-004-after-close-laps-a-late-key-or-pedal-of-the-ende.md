---
id: MODEL-A-004
title: close_laps の後、終わった周回の遅れて届いたキーやペダルが、次の周回にまだイベントがないとき、その周回に戻ってしまう
area: MODEL-A
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# MODEL-A-004: close_laps の後、終わった周回の遅れて届いたキーやペダルが、次の周回にまだイベントがないとき、その周回に戻ってしまう

## Where

crates/meloet-model/src/record.rs:249-281 (`close_laps` は周回を終えるが、次の周回は開かない)
crates/meloet-model/src/record.rs:136-140 (キーは `self.laps.len() - 1`、つまり録音器にある最後の周回に入る)
crates/meloet-model/src/record.rs:370-379 (`control`: 遅れたコントローラーの動きは `last` に入る)
crates/meloet-model/src/record.rs:335-349 (`enter_lap` が周回を追加する唯一の場所で、呼ぶのは `push` と `finish` だけ)

## What happens

ループと「重ねる」を使うとき、コアは聞こえている再生位置が次の周回に入ると、`Recorder::close_laps(upto)` を呼びます（`Play::pass_over`、crates/meloet-core/src/play.rs:1857）。録音器は、その周回のイベントが届いたときにだけ周回を追加します。閉じた瞬間には、普通は新しい周回のキーがまだ届いていないので（使う人のキーは聞こえている位置で刻印されます）、`laps` の最後の周回は、いま閉じたばかりのものです。

閉じた周回の最後の瞬間に押されたキーで、閉じた後にコアへ届いたもの（エンジンは 1 ブロック遅れて報告します）は、「いま進んでいる周回」ではなく、閉じた周回に入ります。
- 遅れたキー: 閉じた周回にまたノートが入り、次の `close_laps` がそれを 1 つのテイクとして出します。「重ねる」では各テイクが独自の手になるため、そのノート 1 つだけを持つ余計な「録音」の手が増えます（「重ねる」をちょうど入れたところで `add_recording` が周回を取る場合は、余計なテイクが増えます）。
- 遅れたペダルアップ（またはベンド、モジュレーション）で、遅れたノートがないもの: それは閉じた周回の `controls` に入ります。その周回にはノートがないため、次の `close_laps` はテイクを出さず、それを消します。ペダルアップの点は曲から失われます（周回 0 のテイクには、最後の瞬間のペダルアップがなくなります）。次の周回は、録音器が保持している値がその動きを受け取るので、やはりペダルアップから始まります。

新しい周回のイベントが閉じる前に来ていた場合は、同じ遅れたキーは新しい周回に入ります（テスト `closed_passes_are_given_once_and_late_keys_go_on` が期待する通りです）。つまり、結果は、新しい周回の最初の 15 ms に使う人が弾いたかどうかで変わります。

## What should happen

`close_laps` の説明: 「終わった周回に遅れて届いたキーは、いま進んでいる周回に入る」。ペダルのテスト `a_pedal_of_a_pass_goes_there_though_notes_of_the_next_pass_came_first`（「周回が終わったら、その後に遅れて届いたものは、いま進んでいる周回に入る」）は、コントローラーの動きについても同じことを期待しています。

## How to trigger

コードを読んで確認したもので、実行はしていません。モデルでは次の通りです。
```
let mut r = Recorder::new(BAR, Some(bars_2_3), HeldControls::default());
r.push(on(B + 100, 0, 60)); r.push(off(B + 200, 0, 60));
r.close_laps(0);                     // laps = [pass 0], all closed
r.push(on(2 * B - 20, 0, 65));       // late key of pass 0
r.push(off(2 * B - 10, 0, 65));
r.push(on(B + 10, 1, 64)); r.push(off(B + 60, 1, 64));
r.close_laps(1)                      // gives 2 takes: [65 alone], [64]; expected 1 take [64, 65]
```
アプリでは、「重ねる」をオンにしてループ録音し、ループの終わりの数 ms 前にキーを押し、次の周回の最初の瞬間には他に何も弾かないでください。

## Cause

`close_laps` は、閉じた周回の中身を消しますが、周回を `laps` の最後の項目として残します。`push` はすべてのキーを `self.laps.len() - 1` に入れ、`control` は閉じた周回を指す動きを、その周回がまだ開いているか（`number >= open_from`）確かめずに `last` に入れます。

## Suggested fix

`close_laps` の最後で、`laps` の最後の周回の `number <= upto` のとき、`self.enter_lap(upto + 1)` を呼んで、いま進んでいる周回を作る。そうすると、遅れたキーや動きは、説明の通りにそこに入ります。プロトコルやファイルの形式は変えません。上の場合をレコーダーのテストに加えてください。

## Verification

自分で実行したテストで確認しました（作業用の worktree で `cargo test -p meloet-model --lib verify_model_a_004`）。「How to trigger」の手順では、`close_laps(1)` で 2 つのテイク（`[(7660, 10, 65)]` と `[(10, 50, 64)]`）になります。`close_laps` の説明とテスト `closed_passes_are_given_once_and_late_keys_go_on` は、両方を含む 1 つのテイクを約束しています。コアは次の周回の聞こえている位置が来ると、すぐ `Play::pass_over`（`play.rs:1844`）から `close_laps` を呼ぶので、この順序は起こりえます。

訂正: ペダルの半分は書いたより小さいものです。閉じた周回の遅れたペダルアップは、その周回から落ちますが、`HeldControls` はそれを受け取るので、次の周回はペダルを踏んだ状態で始まりません（私のテスト `verify_model_a_004_pedal` は通りました。周回 1 の頭に `CC 64 = 127` はありません）。失われるのは、閉じた周回のテイクの最後の瞬間にあったペダルアップの点だけです。重大度 `low` はそのままです。

修正の計画: `crates/meloet-model/src/record.rs` の `close_laps`。ループの後で、`laps` の最後の項目の `number <= upto` のとき、`self.enter_lap(upto + 1)` を呼ぶ。上の手順を `record/tests.rs` に加える。その後 `finish` も確認する。このように入った空の周回は、空のテイクを作ったり、ノートのある周回として数えられたりしてはいけない。

## Fix

一部を修正し、コントローラーの半分は元のままにしました。`crates/meloet-model/src/record.rs` の `Recorder::push`: 終わった周回に遅れて押されたキーは、まずいま進んでいる周回（`enter_lap(open_from)`）に入るので、次の周回のイベントが閉じる前に届いていなくても、そこに入ります。検証者の計画（`close_laps` の最後で `enter_lap(upto + 1)`）は使いませんでした。それでは、終わった周回のペダルアップがまだ届く途中なのに、次の周回の頭を書いてしまい、遅れたペダルアップが次の周回全体をペダルを踏んだ状態のままにしてしまうからです（Fable も同意）。コントローラーの動きは、以前と同じです。いま進んでいる周回がまだないとき、遅れた動きは終わった周回に残り、その点は失われます。次の周回は動いた後の値から始まります。進んでいる周回があるときは、動きはその tick でそこに入り、その周回の頭は古い値のままです（`a_pedal_of_a_pass_goes_there_though_notes_of_the_next_pass_came_first` で意図通りと確かめています）。どちらが正しいかは、リードが決めます（`docs/bugs/later/FIX-fix-model-a.md`）。

テスト: `record::tests::late_keys_go_on_when_nothing_of_the_next_pass_came_first`（meloet-model。遅れたペダルアップが次の周回の頭をペダルを踏んだ状態にしないことも確かめます）。
