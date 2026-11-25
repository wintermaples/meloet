---
id: UI-ROLL-009
title: 書く道具は、曲の再生、録音、カウントイン中にも、書いたノートやコードを鳴らす
area: UI-ROLL
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# UI-ROLL-009: 書く道具は、曲の再生、録音、カウントイン中にも、書いたノートやコードを鳴らす

## Where

- `ui/src/components/pianoroll/drag.ts:241` (コードのスタンプ) と `:250` (一つのノート) — `RollDrag.down` の書く枝は、`i.mayAudition()` を確かめずに `i.audition.play(…)` を呼ぶ。
- 比べる先: `ui/src/components/pianoroll/drag.ts:227` と `:317` (既存のノートを押す、動かす: `if (i.mayAudition())` のときだけ)、および `ui/src/components/pianoroll/audition.ts:20-21` (`notesMayAudition`: 止まっているときだけ)。

## What happens

「書く」を選んで、ノートの領域の空いた所を押すと、`audition` が (選んだコードのすべてのノートとともに) 送られ、曲が再生中、録音中、カウントイン中でも鳴ります。鳴り続けるのは、ボタンを離すまでで、曲の上に重なって鳴ります。既存のノートを押す、または動かすときは、そのような時には鳴りません。録音中やカウントイン中はコアはノートを書きません (編集は使用中だと断られる) が、それでも鳴ります。

## What should happen

仕様 3 「Playing sounds in the piano roll」: 「… Nothing sounds when selecting with a box, on Mod A, when choosing a row in the list, or during play and recording.」(… 枠で選ぶとき、Mod A のとき、一覧で行を選ぶとき、再生中と録音中は何も鳴らない)。録音中に鳴るのは、左の鍵盤の列だけと書かれている。`RollInput.mayAudition` の説明は「Whether to play when a note is pressed or moved (only when stopped)」(ノートを押す、動かすときに鳴らすかどうか (止まっているときだけ))。

## How to trigger

コードを読んで確かめた内容で、実行はしていません。曲を再生する (Space)。ピアノロールで「書く」を選ぶ。ノートの領域の空いた所を押し続ける。

## Cause

書く道具の二つの `audition.play` の呼び出しには、ほかのノートの音と同じ `mayAudition()` の確認がない。

## Suggested fix

書く枝の `i.audition.play(…)` の呼び出しを、`i.mayAudition()` が真のときだけにする (ノートを書くことはそのまま)。プロトコルの変更なし。

## Verification

コードを読んで確かめた (実行していない)。`drag.ts:241` と 250 は、書く枝で `i.mayAudition()` の確認なしに `i.audition.play(…)` を呼ぶ。押す枝と動かす枝には確認がある (227、317)。`mayAudition` は `notesMayAudition(app) && !liveMode` (`Roll.svelte:230`。止まっているときだけ)。仕様 3 の 96 行: 「Notes: a note sounds when pressed (until released; stopped at the length of the note), each time its pitch changes while dragging, and when it is moved with ↑ ↓ (for about 0.3 s). Nothing sounds when selecting with a box, on Mod A, when choosing a row in the list, or during play and recording.」(ノート: 押すと鳴る (離すまで。ノートの長さで止まる)。ドラッグ中に音の高さが変わるたびに鳴る。↑ ↓ で動かすと鳴る (約 0.3 秒)。枠で選ぶとき、Mod A のとき、一覧で行を選ぶとき、再生中と録音中は何も鳴らない)。録音中に鳴ると書かれているのは、鍵盤の列だけ。

修正案 (提案どおり): `ui/src/components/pianoroll/drag.ts` で、書く枝の二つの `i.audition.play` の呼び出しを `if (i.mayAudition())` で囲む。書いているノートをドラッグしている間の音の高さ変化の音 (340 行あたり) も同じ確認があるか確かめる。テスト: 再生中に書く → `audition` のインテントは出ない。

## Fix

プロトコルの変更なし。

- `ui/src/components/pianoroll/drag.ts`: 書く道具の二つの `audition.play` の呼び出し (一つのノートとコード) は `mayAudition()` のときだけ。書くことはそのまま。ノートを動かすときの音の高さの変化の音は、もとから確認がある。
- テスト: `ui/tests/pianoroll.drag.test.ts` の「writes the note but does not sound it (only when stopped, as pressing a note)」。鳴ることを期待していた書く道具の二つのテストは、正しい動きに合わせて変えた (`auditionOn = true` にする。止まっているとき)。
- `docs/architecture.md` 16 「Audition」: 書く道具は、止まっているときだけ鳴る。
