---
id: UI-MIX-005
title: 範囲外の dB を打ちこんだフェーダーかセンドで、Rust がその値を元の値に丸めると、打ちこんだ数字が表示されたままになる
area: UI-MIX
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-MIX-005: 範囲外の dB を打ちこんだフェーダーかセンドで、Rust がその値を元の値に丸めると、打ちこんだ数字が表示されたままになる

## Where

ui/src/components/mixer/control.svelte.ts:118-125 (`submit` は、打ちこんだ値で `keyed = { value: v }` を設定する)
ui/src/components/mixer/control.svelte.ts:38-41 (`keyed` は、曲の値が変わったときにだけ消える)
ui/src/components/mixer/scale.ts:63-68 (`parseDb` は、範囲外の値を Rust に任せる)

## What happens

フェーダーかセンドの箱に dB の値を打ちこむと (Enter、または数字をダブルクリック)、曲の値が変わるまで、打ちこんだ数字が `keyed` として残ります。Rust は範囲外の値を範囲の中へ入れます (`Gain::from_db`、crates/meloet-model/src/mixer.rs:29: +6 より上は +6、−60 より下は −∞)。その結果が、そのストリップがすでに持っている値と同じだと、曲は変わらず (`unless_same`)、状態も返ってこず、`keyed` は残ります。するとストリップは、打ちこんだ数字を表示し、ハンドルもその数字の横に置きます。ところが、音と曲は古い値のままです。例: フェーダーが +6.0 のときに 12 と打ちこむと「12.0」と表示される。センドが −∞ のときに −80 と打ちこむと「−80.0」と表示され、バーは空のまま。

## What should happen

画面は Rust の状態の写しです (CLAUDE.md、architecture 3、決まり 5: 画面が先に動いてよいのは、打ちこんでいる文字と、ドラッグ中の見た目だけ)。Enter のあと、ストリップは、Rust の答えが変えた値の場合と同じく、曲の値を表示すべきです。

## How to trigger

コードを読んで確かめたもので、実行はしていません。ミキサーを開き、フェーダーを一番上 (+6.0) まで上げる。その数字をダブルクリックし、12 と打ちこみ、Enter を押す。フェーダーを動かすまで、数字は「12.0」のままになる。

## Cause

`submit` は、解析した生の値を `keyed` に保存します。それが消えるのは、`opts.value()` の `$effect` によってだけです。その effect は、Rust の答えで値が変わらなかったときには動きません。

## Suggested fix

`send(opts.make(v, true))` のあとには、打ちこんだ値を `keyed` に残さない (`keyed = null` にする。そうすると画面は曲の値と Rust の答えを表示する)。または、残す前に Rust と同じように値を丸める (−60 未満は −∞、+6 より上は +6)。プロトコルの変更も、新しい文面もありません。

## Verification

コードを読んで確かめたもので、実行はしていません。`Where` の行は訂正済みです。`submit` (`control.svelte.ts:118-125`) は、解析した数を `keyed` に残します。`keyed` が消えるのは、`opts.value()` の effect (38-41) によってだけです。`parseDb` (`scale.ts:63-68`) は丸めません。`Gain::from_db` (`crates/meloet-model/src/mixer.rs:29-36`) は、+6 より上を +6 に、−60 より下を −∞ にします。丸めた結果がストリップの持つ値と同じなら、状態は変わらず、`shown` は打ちこんだ数字のままです。関連: UI-PARITY-009 (範囲外の打ちこみの値が何も言わない)。二つは同じ `submit` を直す一回の作業で直せます。

修正計画: `ui/src/components/mixer/control.svelte.ts` の `submit`: 答えを受けたあとは、打ちこんだ値を keyed に残さない。`send(...)` が解決するまでだけ `keyed` を残し、まだこれが同じなら null にする。そうすると、Rust が何をしたかにかかわらず、ストリップは曲の値を表示します。丸めの規則は画面にコピーしません。テスト: フェーダーが +6 のとき 12 と打ちこむ → 答えのあとに表示される値は +6.0。

## Fix

`ui/src/components/mixer/control.svelte.ts` の `submit`: 打ちこんだ値 (`keyed`) は、それに答える rev の状態に届くまでだけ残します。そのあとは、ストリップが曲の値を表示します。テスト: `ui/tests/fix-ui-values.test.ts` の「UI-MIX-005: a typed value Rust keeps as it was」。
