---
id: UI-PARITY-010
title: 再生まわりのテンポと拍子の欄は、カウントイン中や録音中にも開き Enter を受けるが、テンポの帯の欄は開かない
area: UI-PARITY
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-010: 再生まわりのテンポと拍子の欄は、カウントイン中や録音中にも開き Enter を受けるが、テンポの帯の欄は開かない

## Where

ui/src/components/Transport.svelte:446-470 (テンポと拍子の `NumberField` は、使えるかどうかの確認を受けない)
ui/src/components/Transport.svelte:294-297 (`commitClamped` は、設定の命令のあとに `ClampValue` を送る。Rust がそれをどう扱うかは問わない)
ui/src/components/NumberField.svelte:89-98 (読み値は常に `m-typable` で、常に開く)
正しく作られている兄弟: ui/src/components/timeline/TempoBand.svelte:281-287 と :333-340 (`class:m-typable={!busy}`、`if (!busy) editing = ...`。その :29 のコメントは「録音中またはカウントイン中は、選ぶだけ(コアはそれしか受け付けない)」)
Rust 側: crates/meloet-core/src/play/timeline.rs:192-195 と :222-224 (`set_tempo` / `set_meter` は `busy()` のとき、すぐに戻る)

## What happens

カウントイン中や録音中でも、再生まわりのテンポと拍子の数字は、マウスを乗せると入力できる場所として見え(`m-typable`)、押すと入力の欄が開き、Enter も受けます。Rust は `SetTempo` / `SetMeter` を捨てるため、何も変わりません。入力した値が範囲外だった場合(「400」)、画面は `ClampValue` も送り、状態行には、何も設定されていないのにテンポが 300.00 に設定されたと出ます。テンポの帯(テンポの点の数字、拍子の旗)の同じ値は、入力できるようには見えず、そのときは開きません。

## What should happen

同じ値は、どこでも同じように振る舞うべきです。カウントイン中や録音中は入力できない。TempoBand.svelte がそうしており、Rust も強制しています。7-readings の 77 行目では、入力できると分かる見た目は「その数字が入力できないときは出さない」です。

## How to trigger

コードを読んで確認したもので、実行はしていません。
1. ● を押して録音を始める(またはカウントイン中にする)。
2. 再生まわりのテンポの数字を押す: 欄が開く。
3. 400 と入力して Enter: `readTempo` が 300 に丸め、`commitClamped` は `setTempo`(`set_tempo` が `busy()` のため捨てられる)と `clampValue`(Rust は `ValueClamped` を表示する。仕様 6 の「いちばん近い端に設定しました」の文面)を送る。
4. 比べる: 録音中に、テンポの帯の点の数字をダブルクリックする: 何も開かない。

## Cause

再生まわりは、その NumberField に「使えない」状態を渡していません。NumberField には、入力できないと見せる方法がありません。

## Suggested fix

NumberField に `disabled` の prop を与えます(`m-typable` なし。`open` は何もしない。読み値の見た目は今のまま)。そして再生まわりで、テンポと拍子に、録音中・カウントイン中の状態(`recordingNow`)を渡します。テンポの帯が `busy` を渡すのと同じです。プロトコルや文面は変えません。使えない間の読み値の見た目は、テンポの帯が示すような、ただの読み出しの見た目です。

## Verification

コードを読んで確認しました(実行はしていません)。`Transport.svelte:446-470`: テンポと拍子の `NumberField` は使えない状態を渡されず、`NumberField.svelte:90` は常に `m-typable` で、開きます。`set_tempo` / `set_meter` は `busy()` の間は `None` を返します(`crates/meloet-core/src/play/timeline.rs:192-195`、222-224)。`commitClamped`(`Transport.svelte:294-297`)は、それでも `clampValue` を送るため、範囲外の文字では、設定されていないのに設定されたと状態行に出ます。テンポの帯の欄は、使えない間は入力できません。7-readings の 77 行目(入力できる数字へのマウスオーバー): 入力できない数字には出さない。

UI-ROLL-008(今は UI-PARITY-005 が持つ)、UI-ARRANGE-009、UI-ARRANGE-013 と同じ元の原因です(録音中やカウントイン中に使えそうに見えるが、Rust が拒否する部品)。

修正案(提案どおり): `ui/src/components/NumberField.svelte` に `disabled` の prop(`m-typable` なし。`open` は何もしない。読み値の見た目は今のまま)を置き、`ui/src/components/Transport.svelte` でテンポと拍子に、再生まわりの録音中・カウントイン中の状態を渡す。位置の欄も確認する(`locate` も busy の間は拒否される。`play.rs:928-931`)。今開くなら、同じ prop を渡す。プロトコルや文面は変えない。

## Fix

コミットはまだしていません(リーダーがコミットします)。

- `ui/src/components/NumberField.svelte`(別の修正者のファイル。小さく保つ): `disabled` の prop。押しても欄は開かず、`m-typable` もなく、ホバーの枠もなく、`cursor: default`、`aria-disabled="true"`(再生まわりが、今は使えない部品に示す印)。この prop がなければ、マークアップは今までどおり。
- `ui/src/components/Transport.svelte`: テンポと拍子に `disabled={recordingNow}`。位置にも同じく(検証が求めたとおり、`locate` も busy の間は拒否されるため)。

テスト: `ui/tests/transport.test.ts` —「録音中またはカウントイン中は、テンポ、拍子、位置が開かず、入力できるようにも見えない(コアはどれも受け付けない。テンポの帯と同じ)」。
