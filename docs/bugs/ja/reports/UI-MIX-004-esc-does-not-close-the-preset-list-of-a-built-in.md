---
id: UI-MIX-004
title: 組み込みエフェクトのプリセット一覧は、フォーカスが中にないと Esc で閉じない
area: UI-MIX
kind: ui-ux
severity: medium
status: fixed
found: 2026-11-25
---

# UI-MIX-004: 組み込みエフェクトのプリセット一覧は、フォーカスが中にないと Esc で閉じない

## Where

ui/src/components/params/PresetMenu.svelte:58-68, 72 (Esc を読むのは `.menu` の div の `onkeydown` だけ)
ui/src/components/params/PresetMenu.svelte:62 (名前の入力欄での最初の Esc は欄を消し、フォーカスが一覧から出る)
ui/src/components/params/BuiltinList.svelte:132 (一覧はここで開く。開く 「プリセット ▾」 のボタンは、メニューの外にある)

## What happens

組み込みエフェクトのプリセット一覧は、キーボードのフォーカスが一覧の中にあるときだけ Esc で閉じます。「プリセット ▾」を押すとフォーカスはそのボタン (またはもとの場所) に残ります。それは一覧の外なので、Esc はアプリのキー処理へ行きます。ドラッグもメニューもないので、一般の `back` インテントを送り、一覧は開いたままになります。「いまの設定を残す…」の名前の入力欄では、最初の Esc で行が元に戻り (`naming = false`)、入力欄が消えるので、フォーカスはページに落ち、二度目の Esc も一覧に届きません。

## What should happen

仕様 3 (round 12 M1、3-bands.md:409): 「Esc puts the row back, Esc again closes the list」(Esc で行を戻し、もう一度 Esc で一覧を閉じる)。PresetMenu.svelte のヘッダ: 「Esc closes the list」(Esc で一覧を閉じる)。同じ仲間の小さな窓は `menuOpened` (lib/menu.ts) で登録しているので、共通の Esc の順番が、フォーカスがどこにあっても閉じます: アルペジオの窓 (InputMeter.svelte:96)、ミュートセットの名前の入力欄 (MuteSetBox.svelte:65)。

## How to trigger

コードを読んで確かめた内容で、実行はしていません。1. ミキサーで組み込みのリバーブを選ぶ。2. パラメータの欄の「プリセット ▾」を押す。3. Esc を押す: 一覧は開いたままになり (`back` が送られる)。または: 2. 一覧を開き、「いまの設定を残す…」を押す。Esc を押す (行が戻る)。もう一度 Esc を押す: 一覧は開いたまま。

## Cause

Esc の処理は、一覧そのものの要素にある keydown の処理。一覧は、共通の Esc の順番に登録されておらず、開いたときや名前の入力欄が消えた後にフォーカスを受けない。

## Suggested fix

開いている間、一覧を `menuOpened(onclose)` (lib/menu.ts) で登録する。アルペジオの窓と同じ。そうすると Esc は最初に closeMenus で閉じられる。または、開いたとき (名前の入力欄が消えた後にも) 一覧にフォーカスを置く (tabindex=-1)。プロトコルの変更なし、文面の追加なし。

## Verification

コードを読んで確かめた (実行していない)。訂正した行: Esc を読むのは `.menu` の div の `onkeydown` だけ (`PresetMenu.svelte:58-68`、72)。一覧は `BuiltinList.svelte:132` で、その外のボタンから開く。一覧が開いたときにフォーカスを置く処理はなく、`naming = false` (62) が、フォーカスのある名前の入力欄を消す。一覧は `menuOpened` (`lib/menu.ts`) に登録されていないので、`handleKeydown` の `closeMenus()` はそれを知らず、Esc は `back` へ進む (これは帯を折りたたむか、使う人がいた場所を出る)。仕様 3 の 409 行: 「Esc puts the row back, Esc again closes the list」。WebKitGTK で「プリセット ▾」を押したとき、フォーカスがボタンに残るか body に行くかは確かめていない。どちらでも一覧の外。

UI-SHELL-001、-002、-003、-004、-005、UI-PARITY-011、UI-PARITY-015、UI-MIX-004 は一つの根から出ている。手前の部品 (ダイアログ、開いている一覧、フォーカスのある操作部品) が使うべきキーが、文書全体のキー処理 (`ui/src/keys/attach.ts` の `handleKeydown`) へ落ちる。そこは、手前の部品があることを知らない。一つの作業にまとめて直す。`handleKeydown` に一つの規則を入れる: (1) 「どのダイアログが手前か」を一か所で確かめ、すべての文脈 (`outputChooser`、`shelf`、`history`) と Enter / Esc の枝で使う。順番は `App.svelte` で描かれる順。(2) `ev.defaultPrevented` のとき早く戻る (キーを取った部品がそう知らせる)。これで `body` へ移された一覧も扱える。そうすれば、各部品は自分のキーだけを扱えばよい。

この報告の修正案 (提案どおり): `ui/src/components/params/PresetMenu.svelte` で、開いている一覧をマウントされている間 `menuOpened(onclose)` で登録する (`InputMeter.svelte:96` と `MuteSetBox.svelte:65` と同じ)。そうすると三回目の Esc は、フォーカスがどこにあっても一覧を閉じる。「名前の入力欄で最初の Esc は行を戻す」は一覧自身の処理に残す。`naming = false` の後は一覧にフォーカスを置く (tabindex −1)。そうすると二度目の Esc は一覧のものになる。

## Fix

`ui/src/components/params/PresetMenu.svelte`: 開いている一覧は、マウントされている間 `menuOpened` (`lib/menu.ts`) に登録される。そのため、メニューの側の Esc の順番 (`keys/attach.ts` の `closeMenus`) は、フォーカスがどこにあっても一覧を閉じる。名前の入力欄での Esc は、まだ先に行を戻す (一覧自身の処理が止める)。次の Esc は `closeMenus` で一覧を閉じるので、一覧自体にはフォーカスを置かない。テスト: `ui/tests/round12.presets.test.ts` の「UI-MIX-004: Esc closes the list wherever the focus is; in the name box the first Esc puts the row back」。
