---
id: UI-ARRANGE-009
title: テイク切りかえの札の前後ボタンは、録音中やカウントイン中も押せるが、そのとき Rust はテイクの切りかえを受け付けない
area: UI-ARRANGE
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-009: テイク切りかえの札の前後ボタンは、録音中やカウントイン中も押せるが、そのとき Rust はテイクの切りかえを受け付けない

## Where

ui/src/components/Arrange.svelte:762-775 (`switcher`: 再生の状態にかかわらず表示される)
ui/src/components/Arrange.svelte:1988-1999 (◂ / ▸ は常に `stepTake` を送り、使えない状態にはならない)
crates/meloet-core/src/play.rs:1295-1298 (`step_take`: `if self.busy() … return None`)

## What happens

録音中やカウントイン中も、選んだクリップの上のテイク切りかえの札は、◂ と ▸ を使えるボタンとして見せています。押すと `stepTake` が送られますが、コアは busy の間はそれを拒否し、何も言わないため、何も起きません。

## What should happen

アーキテクチャ 10(「録音中とカウントイン中の操作」): 「受け付けない: … テイクの切りかえ …」。今は使えない部品は、使えない状態で表示します(CLAUDE.md の「まだ中身のない部品は、取り除かず、使えない状態で表示する」。仕様 2「部品と状態」)。兄弟の部品は、すでにそうしています。同じアレンジの中で、クリップの右クリックメニューは busy の間、「次のテイク」と「前のテイク」を使えなくし(`Arrange.svelte:1593-1594`、`change = !busy`)、メニューの行も busy の間は使えなくし(`menu/menus.ts:279`、`canStepTake` は `!busy` が必要)、ピアノロールの札は、そのクリップが録音中は ◂ ▸ を使えなくします(`pianoroll/Toolbar.svelte:225, 234`)。

## How to trigger

コードを読んで確認したもので、実行はしていません。2 つのテイクがあるクリップを選ぶ。別のトラックで録音を始める(R)。札の ▸ を押す: 何も変わらず、何も言われない。

## Cause

切りかえボタンには、`busy` の確認も、使えない状態の見た目もありません。

## Suggested fix

`busy` の間、◂ と ▸ を使えない状態にする(アレンジの他の使えないボタンと同じく、`aria-disabled` と使えない見た目で、送らない)。プロトコルや文面は変えません。

## Verification

コードを読んで確認しました(実行はしていません)。`Arrange.svelte:1990, 1996` は常に `stepTake` を送ります。`step_take` は `busy()` のときすぐ戻ります(`crates/meloet-core/src/play.rs:1295-1298`)。同じ操作の右クリックメニューの行とメニューの行は、busy の間は使えず、ピアノロールの札はそのボタンを使えなくします。

UI-ROLL-008(今は UI-PARITY-005 が持つ)、UI-ARRANGE-013、UI-PARITY-010 と同じ元の原因です。録音中やカウントイン中に使えそうに見えるが、Rust が意図を拒否する部品です。それぞれは別のファイルで直しますが、一つの規則で直してください。その規則とは、同じ `busy` を読むこと(再生が録音の意味で止まっていないこと。`ui/src/keys/attach.ts` の `songBusy` と、`Arrange.svelte` にすでにある `busy` と同じ)、使えない状態を見せること、そして何も送らないことです。

修正案: `ui/src/components/Arrange.svelte` で、◂ と ▸ に `disabled={busy}`(または、アレンジの他の使えないボタンと同じく `aria-disabled` と、送らないこと)と使えない見た目を与える。UI-ARRANGE-008 と同じ二つのボタンなので、一緒に直す。プロトコルや文面は変えない。

## Fix

`ui/src/components/Arrange.svelte`: テイク切りかえの札の ◂ と ▸ に `disabled={busy}`(録音中またはカウントイン中)と使えない見た目(UI-ARRANGE-008 と一緒)を与えました。使えないボタンは何も送りません。テスト: `ui/tests/arrange.test.ts` の「録音中またはカウントイン中は、切りかえの札の ◂ ▸ が使えず、何も送らない」。
