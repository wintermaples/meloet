---
id: UI-ARRANGE-007
title: オートメーションの点を追加すると、画面は拍にそろえ、Rust はピアノロールのグリッドへ再びそろえ、Shift を無視する
area: UI-ARRANGE
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-007: オートメーションの点を追加すると、画面は拍にそろえ、Rust はピアノロールのグリッドへ再びそろえ、Shift を無視する

## Where

ui/src/components/automation/autoGeom.ts:348-352 (`addTick`: 拍の頭。Shift のときはティックをそのまま)
ui/src/components/automation/AutoLane.svelte:109-115 (`addReading`: 読み値はその場所と、線のその場所の値を示す)
ui/src/components/automation/AutoLane.svelte:172-173 (その tick で `addAutoPoint`)
crates/meloet-core/src/play/automation.rs:587-615 (`add_auto_point` → `on_grid`: 常に、ピアノロールのグリッドである `session.roll.grid` へ再びそろえる)

## What happens

オートメーションの行の空いている場所を押すと、その場所の拍の頭を示す読み値「位置 ・ 値」が出る(Shift を押していれば、ちょうどのティック)。そして、そのティックで `addAutoPoint` を送る。すると Rust は、その代わりに点をピアノロールのグリッドに置く。そのグリッドが拍より粗いとき、または拍を含まないとき(グリッドが 1 小節、1/2、1/4 の三連符、または 1/4 のグリッドでの x/8)、点は読み値が示した場所とは別の場所に入り、そこでとった値が付く。そして Shift でもグリッドの間には点を足せない。同じ行で点を動かすときは拍にそろうので (`dragPoints`、Rust は送られた `dTick` をそのまま使う)、同じ行で足すことと動かすことは二つの違うグリッドを使う。

## What should happen

両側で一つの規則にし、読み値のある場所に点が入るようにする。仕様書 3 (Y16): 「帯の空いている場所を押すと点が足される(押した位置の線の値で) … 横の位置は(グリッドにそろう)」。仕様書 4: Shift + ドラッグはそろえない。画面の規則 (拍、Shift ではティック) は、そのテストが確かめているもの (`ui/tests/automation.test.ts:112`、`:140`) である。

## How to trigger

コードから読んだもので、実行はしていない。ピアノロールのグリッドを 1 小節にする(「ノート ▸ グリッド ▸」)。アレンジで音量の行を足し、小節 5 の拍 3 でその空いている場所を押す。読み値は「5. 3. 0000 …」と言い、点は小節線 (小節 5 か 6) の上に出る。Shift を押しながら拍の間を押すと、点は依然としてグリッドの上に入る。

## Cause

`add_auto_point` は常にピアノロールのグリッドで `on_grid` を実行する。一方、画面はすでに自分の(拍の)規則でそろえていて、「そろえない」ために Shift を用意している。

## Suggested fix

オートメーションの点がどのグリッドを使うかは、リードが決める必要がある。画面が検査された規則を保つ最小の変更: `add_auto_point` が送られたティックをそのまま受け取るようにする (画面がそろえ済み)。`move_auto_points` が送られた `dTick` を受け取るのと同じである。`add_auto_point` の doc コメントもそれに合わせて変える。プロトコルや文面の変更はない。

## Verification

コードを読んで確認したもので、実行はしていない。`add_auto_point` (`crates/meloet-core/src/play/automation.rs:587-604`) は常に `on_grid` を実行し、それはピアノロールのグリッド (`self.song.session.roll.grid`) を使う。画面はすでにティックを拍の頭に置いているか、Shift のときはそのまま残し (`autoGeom.ts:348-352`)、その場所を読み値に示している。

どちらの側に従うかは 7-readings A-21 が決めている(よって `needs-lead` ではない): 「追加・移動される点のそろえ方 | 拍の頭。Shift をおすとティック。 | mock R10-AutoEdit」。画面はそれに従い、`move_auto_points` はすでに画面の `dTick` をそのまま受け取る。違うのは `add_auto_point` だけである。

Fix plan: `crates/meloet-core/src/play/automation.rs` で、`add_auto_point` が送られたティックをそのまま受け取るようにする(`min(MAX_TICK)` だけ)。ほかに使うものがなければ `on_grid` を消す(まず grep する。AI の連携の追加が同じ関数を呼ぶかもしれない。AI のツールがグリッドに頼っているなら、そちらの経路だけ、そろえを残す)。`add_auto_point` の doc コメントと、グリッドを期待する Rust のテストを変える。プロトコルや文面の変更はない。

## Fix

まだコミットしていない(リードがコミットする)。

- `crates/meloet-core/src/play/automation.rs`: `add_auto_point` は送られたティックをそのまま受け取る (モデルの `add_point` は今も `MAX_TICK` で抑える)。`move_auto_points` が `dTick` を受け取るのと同じである。`on_grid` は消した (ほかに使うものがなかった)。doc コメントもそう書く。ほかの呼び出し元も確認した: AI の連携は `play/ai.rs` (`Put`) で点を書き、この intent は使わない。`AddAutoPoint` を送る Rust のテストは拍の上のティックを使う。下のひとつを除く。

テスト: `crates/meloet-core/src/play/automation/tests.rs` の `lanes_and_points_are_steps_and_the_selection_comes_back_with_undo` は、点が送ったとおり `Q / 4 + 10` に来ることを期待するようになった(以前はピアノロールの 1/16 グリッドの `Q / 4` を期待していた)。
