---
id: UI-ARRANGE-006
title: マーカーとテンポの点を Shift を押してドラッグすると、拍に合わない場所を表示するが、Rust は常に拍に合わせる
area: UI-ARRANGE
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-006: マーカーとテンポの点を Shift を押してドラッグすると、拍に合わない場所を表示するが、Rust は常に拍に合わせる

## Where

ui/src/components/timeline/MarkerRow.svelte:67-68 (`toTick = e.shiftKey ? Math.round(at) : nearestBeatTick(map, at)`)
ui/src/components/timeline/tempoGeom.ts:252-258 (`dragPoint`: `shift ? Math.round(at) : nearestBeatTick(map, at)`)
crates/meloet-core/src/play/timeline.rs:427-434 (`move_marker`: 常に `let tick = self.snapped(tick)`)
crates/meloet-core/src/play/timeline.rs:262-286 (`move_tempo_point`: 点が動くとき、常に `self.snapped(to_tick)`)

## What happens

マーカーの札やテンポの点を、Shift を押したままドラッグすると、画面は吸着しません。札や点はポインターに合わせて tick ごとに動き、その場にある表示は、その位置を示します（たとえば「9. 2. 0480」）。手を離すと、インテントはその tick を運びますが、Rust は最も近い拍に吸着させるので、マーカーや点は、使う人が選んでいない拍へ飛びます。この 2 つのドラッグでは Shift は何もしないのに、画面はそうすると言っています。

## What should happen

画面と Rust が一致するべきです。仕様 4 は、すべてのドラッグについて「ドラッグ中は、グリッドに吸着させない。Shift + ドラッグ — 動く」と挙げています。仕様 3（Y18、Y17）は、マーカーとテンポの点の移動は拍に吸着すると言っています。Rust が Shift を押したときに送られた tick をそのまま使うか、この 2 つのドラッグでは画面が Shift を提供しない（いつも吸着する）かのどちらかで、表示されたものと置かれるものが一致するべきです。アレンジのほかのドラッグは両方で一致しています（クリップの移動は、Rust が送られた量を使う。オートメーションの点の移動は、Rust が送られた `dTick` を使う）。

## How to trigger

コードを読んで確認したもので、実行はしていません。「テンポ ・ 拍子の変更」を開き、Shift を押したまま、テンポの点を拍から少しずらして横にドラッグします。表示には拍からずれた場所が出ます。手を離すと、点は最も近い拍に置かれます。マーカーの行の札でも同じです。

## Cause

`move_marker` と `move_tempo_point` は、画面が何を送っても `snapped()` を呼びますが、画面は Shift のときは吸着をしません。

## Suggested fix

判断はリードが必要です。(a) 画面の Shift の分岐をなくす（いつも `nearestBeatTick`）。今の Rust の動作に合わせる。プロトコルは変わらない。または (b) これらのインテントに「吸着しない」という希望を運ばせ（`MoveMarker` と `MoveTempoPoint` に `snap: bool` のようなプロトコルの欄を加える）、それが偽のとき `snapped()` を飛ばす（architecture 21-15 は「コアは、同じ規則で、もう一度吸着させる」と言っているので、その行を変える必要がある）。

## Verification

コードを読んで確認しました（実行はしていません）。画面は Shift のとき吸着をしません（`MarkerRow.svelte:68`、`tempoGeom.ts:255`）。Rust は常に吸着させます（`play/timeline.rs` の `move_marker`: `let tick = self.snapped(tick)`。`move_tempo_point` も同じ。`snapped` は最も近い拍）。

どちら側に従うかは文書で決まっているので、`needs-lead` ではありません。Rust が正しく、画面を変えます。
- 仕様 3（Y17）: 「テンポは拍の始めでだけ変わることができる」。
- 仕様 3（Y18）: 「動かす: 横にドラッグする（拍に吸着する。表示は位置）。」
- architecture 21-15: 「**拍への吸着は両方がする**: 画面は、ドラッグ中の見た目のために、マップ（`nextBeat`）で吸着し、コアは同じ規則（最も近い拍。ちょうど半分のときは後の拍）でもう一度吸着させる。」
- 仕様 4 の「ドラッグ中は、グリッドに吸着させない — Shift + ドラッグ」の行はグリッドについてのもので、この 2 つのドラッグはグリッドではなく、上の拍の規則を使います。7-readings には、マーカーやテンポの点のドラッグに Shift を与える行がありません（A-20）。一方、A-21 はオートメーションの点について、言葉で Shift を与えています（「拍の始め。Shift を押すと、tick 単位」）。

修正の計画（選択肢 (a)）: `ui/src/components/timeline/MarkerRow.svelte:68` と `ui/src/components/timeline/tempoGeom.ts:255` の `dragPoint` の Shift の分岐をなくす（いつも `nearestBeatTick`）。`dragPoint` の上にあるコメント（「Shift を押すと tick」）も直す。`ui/tests/` で Shift の形を期待しているテスト（`tempoGeom` のテスト）があれば変える。プロトコルは変えない。

## Fix

まだコミットしていません（リードがコミットします）。

- `ui/src/components/timeline/MarkerRow.svelte`: マーカーのドラッグは、いつも最も近い拍に吸着する（Shift の分岐はなくなった）。
- `ui/src/components/timeline/tempoGeom.ts`: `dragPoint` は、いつも最も近い拍に吸着する。その `shift` の引数と「Shift を押すと tick」のコメントはなくなった。`TempoBand.svelte` はもう渡さない。

テスト: `ui/tests/round6.timeline.test.ts` の「Shift を押しても、テンポの点とマーカーは、コアが置くように拍の始めへ行く（仕様 3 Y17、Y18。architecture 21-15）」。形のテスト「drag: sideways to beat starts, …」は、引数 `false` が消えただけです。
