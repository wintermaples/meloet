---
id: UI-SHELL-003
title: 遅れの窓で押した Esc と Enter が、その下の音の設定の窓に働く
area: UI-SHELL
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-SHELL-003: 遅れの窓で押した Esc と Enter が、その下の音の設定の窓に働く

## Where

ui/src/keys/attach.ts:538-540 (`workspace.settings` があるとき、ただの Enter は settingsApply を送る)
ui/src/keys/attach.ts:555-564 (Esc は `back` を送る。`workspace.latencyDialog` は見られていない)
ui/src/components/LatencyDialog.svelte:49-50 (`cancel` は Esc のために書かれているが、呼ぶのはボタンだけ)
crates/meloet-core/src/state.rs:1702-1710 (`Back` は `self.dialog`（音の設定の窓）を閉じるが、遅れの窓は閉じない)
ui/src/App.svelte:98-107 (遅れの窓は、`latencyDialog` が設定されていれば、設定の窓があってもなくても表示される)

## What happens

遅れの窓は、詳しい音の設定の「遅れを測る…」から開かれるので、コアでは音の設定の窓がその下でまだ開いています。
- Esc は `back` を送り、コアは隠れている音の設定を閉じます。遅れの窓は画面に残ります（測っている途中なら、測定は続きます）。2 回目の Esc で帯がたたまれ、アレンジから出ます。遅れの窓は Esc では閉じません。
- Enter は `settingsApply` を送ります。隠れている音の設定が適用されて閉じられ（別の出口が出力の下書きとして選ばれます）、遅れの窓は残ります。

## What should happen

spec 4: Esc は窓を閉じるか、一段戻ります。そして、手前の窓が先に扱われます（spec 3 の Esc の順番）。遅れの窓のコメント（LatencyDialog.svelte:49）は、Esc を「やめる」と書いています。測定中なら測定を止め（`stopLatencyMeasure`）、そうでなければ窓を閉じます（`closeLatencyMeasure`）。Enter は、その下の音の設定に何もしないべきです（この窓の強い方のボタン、または何もしない）。

## How to trigger

コードを読んで確かめました。実行はしていません。
1. Mod , → 「くわしい設定」 → 「遅れを測る…」。遅れの窓が設定の上に開きます。
2. Esc を押す。コアで音の設定は閉じますが、遅れの窓は開いたままです。
   または Enter を押す。`settingsApply` が送られ、遅れの窓は開いたままです。

## Cause

`handleKeydown` には、`state.workspace.latencyDialog` の分岐がありません。Esc は `{ type: "back" }` に落ちます。その core の処理は、まず `self.dialog`（設定）を閉じます。ただの Enter は、遅れの窓の下で設定がまだ開いているので、`state.workspace.settings` の分岐に当たります。

## Suggested fix

attach.ts で、設定の分岐より前に、`state.workspace.latencyDialog` が設定されているとき、Esc は測定中なら `stopLatencyMeasure`、そうでなければ `closeLatencyMeasure` を送ります（窓の `cancel` と同じ。関数は 1 つにして共有する）。そして、ただの Enter は `settingsApply` を送りません（この窓の強いボタンを押すか、何もしない）。プロトコル、文言、見た目は変えません。

## Verification

実行して確かめました（一時の作業ツリー。残していません）。`workspace.settings` と `workspace.latencyDialog` の両方が設定されているとき、`handleKeydown` は Esc に `{ type: "back" }` を、Enter に `{ type: "settingsApply" }` を、ちょうど 1 つずつ送りました。コードを読んで確かめたこと: コアの `Back`（`crates/meloet-core/src/state.rs:1702-1710`）は、スキャンで止まった窓、曲の窓、そのあと `self.dialog`（音の設定）を閉じます。遅れの窓には触れません。`App.svelte:98-107` は、両方が設定されているとき、遅れの窓を設定の代わりに描きます。`LatencyDialog.svelte:49-50` の `cancel` は「(Esc)」と書かれていますが、呼ぶのはボタンだけです。

UI-SHELL-001、-002、-003、-004、-005、UI-PARITY-011、UI-PARITY-015、UI-MIX-004 は、ひとつの根が同じです。手前の部品（窓、開いている一覧、フォーカスのある操作部品）が扱うべきキーが、文書全体のキーの処理（`ui/src/keys/attach.ts` の `handleKeydown`）に落ち、そこは手前の部品を知りません。これらは 1 つの作業としてまとめて直し、`handleKeydown` に 1 つの規則を入れます。(1) 「どの窓が手前か」を見る 1 つの確認を、すべての文脈（`outputChooser`、`shelf`、`history`）と Enter と Esc の分岐で使う。順番は `App.svelte` で窓が描かれる順番に合わせる。(2) `ev.defaultPrevented` のときは早く戻る（キーを受け取った部品がそう知らせる。`body` へ移された一覧もこれで扱える）。そうすれば、各部品は自分のキーだけを扱えばよくなります。

この報告の修正案: `ui/src/keys/attach.ts` で、`state.workspace.latencyDialog` が設定されているとき、Esc は測定中なら `stopLatencyMeasure`、そうでなければ `closeLatencyMeasure` を送ります（窓の `cancel` がすることと同じ。窓と共有する 1 つの関数にする）。ただの Enter は、設定に何も送りません（spec 3 は、この窓に Enter の規則を与えていないので、何もしないのが安全な選択です）。プロトコルも文言も変えません。

## Fix

`ui/src/keys/attach.ts`: 遅れの窓が手前にあるとき、Esc は `latencyCancel(measure)` を送ります（測定中なら `stopLatencyMeasure`、そうでなければ `closeLatencyMeasure`）。ただの Enter は何も送りません。`latencyCancel` は `ui/src/keys/front.ts` にあり、`ui/src/components/LatencyDialog.svelte` の `cancel` もそれを使います。テスト: `ui/tests/keys.front.test.ts` の「UI-SHELL-003: …」。
