---
id: UI-ARRANGE-001
title: オートメーションのレーン: 点のドラッグ後に残す見た目が次のレーンの点に当たり、描画中に例外が出る
area: UI-ARRANGE
kind: bug
severity: high
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-001: オートメーションのレーン: 点のドラッグ後に残す見た目が次のレーンの点に当たり、描画中に例外が出る

## Where

ui/src/components/automation/AutoLane.svelte:37-38 (`points` / `pts`: レーンの新しい `laneId` の点がまだ届いていない間は空)
ui/src/components/automation/AutoLane.svelte:74-80 (`pending` は `$effect` でしか消されない。`shownDrag` は `pending.laneId` を確かめずに `pending.drag` を使う)
ui/src/components/automation/AutoLane.svelte:81-91 (`rects`: `from: d.points.map((i) => pts[i]!)`)
ui/src/components/automation/AutoLane.svelte:153-165 (`up()`: `moveAutoPoints` の後も `pending` を残す)
ui/src/components/automation/autoGeom.ts:214-217 (`draggedPoints`: 古いレーンの番号で `pts[i]!.tick` を読む)

## What happens

オートメーションのレーン (またはマスターのレーン) で点をドラッグして離すと、点が元の位置に戻らないように `pending` がそのドラッグを残します。`MoveAutoPoints` は新しい `LaneId` を作るので (architecture 21-5: 「点が変わるたびに新しい id」)、移動への答えの状態の差分でレーンの `laneId` が新しくなり、その点はまだ取りに行かれていません (`LaneStore.get` は中身を読まれたときだけ呼ばれ、先読みはしません)。その差分の反映で `pts` は `[]` になりますが、`shownDrag` はまだ `pending.drag` (`moved: true` つき) のままなので、`A.draggedPoints([], drag)` は `undefined` の `pts[i]!.tick` を読んで `TypeError` を投げます。

この例外は、レーンの `use:glSlot` の更新 (描画用のエフェクト) の中で起きます。Svelte 5.57 は利用者の `$effect` より先に描画用のエフェクトを動かすので、`pending` を消す `$effect` (74 行) はまだ動いていません。アプリには `<svelte:boundary>` がないので、例外は batch の外へ出ます。`Batch.#process` がそれを受けて `reset_all(root)` を呼び (アプリ木の全エフェクトを「きれい」な状態に戻す)、もう一度投げ直します (svelte 5.57.3、`ui/node_modules/svelte/src/internal/client/reactivity/batch.js:366-378`、`reset_all` は 1330 行、`error-handling.js` の `invoke_error_boundary` は境界がないので再度投げる)。コードを読んで分かる結果は次のとおりです。

- 点をドラッグするたびに、実アプリのコンソールに捕まらない `TypeError` が出る。
- このレーンより後ろの部品と、その batch のすべての `$effect` (たとえば `AutoClock` の位置の更新、ミキサー、取り消しの履歴の数) は、何かが再び汚れるまで、その状態を取り込まない。
- 新しい点が届く (`autoState` の `version` が上がる) と、レーンは描き直されます。`pending` がまだ残っていると (消す `$effect` がきれいな状態に戻され、差分が届くまでに `pending`、`lane`、`current.rev` が書き換えられていない場合。`send()` が差分より先に解決したときに当たる) 、古い `pending.drag` が**新しい**点の上に重なります。動かした点がもう一度動いて見え (`dTick` / `dValue` がもう一度足される)、動いた位置にアクセントの枠が出ます。別の状態の変化で消す `$effect` が動くまで続きます。`send()` が差分の後に解決すると、その `.then` が `pending` を書き、消す処理がすぐ動くので、見えるのは例外だけです。

フェイクのブリッジは `moveAutoPoints` に何もせず (`ui/src/bridge/fake.ts:3249-3252`、新しいレーンの id がない)、テストもマウントされたレーンの点をドラッグしないので、テストでは見えません。

ドラッグ中 (離した後だけでなく) にも、点を押している間にレーンの `laneId` が変わると (たとえば点を押したまま Mod Z を押す、または AI のリンクからの編集)、同じ例外が出ることがあります。`dragPoints` (`autoGeom.ts:203`) と `moveReading` (`AutoLane.svelte:101`) が、新しい一覧に対して `pts[d.grab]!` を読むためです。

## What should happen

離した後に残す見た目は、答えの点が届くまで動かした点を見せ、その後は通常の描画に移るべきで、例外は出ない。(`AutoLane.svelte:68` のコメント「The look kept after release ... so the point does not jump back」の意図どおり。) 隣の `CompBand.svelte:54` は正しく作られています。`pending.clip === clip.id` のときだけ `pending` を使います。

## How to trigger

コードを読んで確かめた内容で、実行はしていません。実アプリ (フェイクの画面ではない) で: あるトラックに音量のレーンを追加し、点を二つ足し、一つの点を横、上、下へ動かして離す。答えが届くと、コンソールに `draggedPoints` 由来の `TypeError: Cannot read properties of undefined (reading 'tick')` が出ます。答え (コマンドの応答) と差分のどちらが先に来るかで、その後は、状態を何かが変えるまで、レーンの線が動かした距離の二倍動いて見えることがあります。

## Cause

`shownDrag = drag ?? pending?.drag ?? null` (79 行) は、`pending.laneId` がまだ `lane.laneId` と同じかを確かめていません。`draggedPoints` と `rects` は、古いレーンの番号で `pts` を引きます。消す `$effect` は、すでに例外を投げた描画用のエフェクトの後で動きます。

## Suggested fix

残した drag は、`pts` ではなく、それを作った点に当てる。`pending.laneId !== lane.laneId` の間は、`pending.laneId` の点 (`peekLanePoints` にはまだ残っている。`LaneStore.keepOnly` は落としたレーンも残す) の上に `draggedPoints` を描き、新しいレーンの点が届いたら (`points?.id === lane.laneId`) `pending` を捨てる。少なくとも、`pending.drag` は `pending.laneId === lane.laneId` のときだけ使い (`CompBand` と同じ)、`draggedPoints` / `dragPoints` は `pts` にない番号を飛ばす。プロトコルも文面も変えない。

## Verification

テストを実行して確かめた (2026-11-25、作業用の scratch worktree、残していない)。vitest のテストで `AutoLane.svelte` をマウントした (svelte 5.57.3、jsdom)。点が三つのレーンを、モックした `peekLanePoints` で渡し、真ん中の点に pointerdown / pointermove / pointerup を送り (`moveAutoPoints` が送られた)、その後、同じレーンで `laneId` だけ新しく、点がないものをレーンの prop に渡して、`flushSync()` を呼んだ。結果: `flushSync` が `TypeError: Cannot read properties of undefined (reading 'tick')` を投げ、スタックに `draggedPoints` があった。この実行では `pending.laneId !== lane.laneId` が成り立っていたので、消す `$effect` は `pending` を捨てたはずだが、例外が先に出た。

順序がこうなる理由 (`ui/node_modules/svelte` 5.57.3 を読んで確認): `use:glSlot={{ kind: "rects", rects }}` は `render_effect` の中で `rects` を読み (`src/internal/client/dom/elements/actions.js:22`)、描画用のエフェクトは利用者の `$effect` より先に動きます。`ui/src` には `<svelte:boundary>` がない (grep で確認) ので、例外は `Batch.#process` の外へ出て、そこで `reset_all(root)` を呼び、もう一度投げます (`reactivity/batch.js:366-378`)。

コードを読んだだけで実行していない内容: 新しい点が届いた後の「もう一度動いて見える」状態 (`send()` が差分より先に解決したときに起きる。報告の論理は、読んだ限りでは正しい)、そして `laneId` が変わったときにドラッグを保ったまま起きる例外 (`dragPoints`、`moveReading`: 空か短くなった新しいリストに対して同じ `pts[i]!`)。

Rust 側も確認した: `auto::move_points` は新しい `LaneId` を返す (`crates/meloet-model/src/automation.rs:425-429`)。だから実際の移動はすべてこの道を通ります。フェイクのブリッジが `moveAutoPoints` に何もしないので、どのテストもこれを見ていませんでした。

重大度は high のまま: 点を動かすたびに、実アプリで捕まらない例外が出て、batch が捨てられる (画面の他の部品がその状態を取り逃す)。

修正案 (合意済み、最小): `ui/src/components/automation/AutoLane.svelte` で、残す見た目を元で安全にする: `const shownDrag = $derived(drag ?? (pending && pending.laneId === lane.laneId ? pending.drag : null))`。保持中の `drag` は、`lane.laneId` がそれを始めたときのものでなくなったら捨てる (drag に laneId を覚えるか、同じ derived の中でキャンセルする)。`ui/src/components/automation/autoGeom.ts` では、`draggedPoints` と `dragPoints` が、番号が点の中にないとき入力をそのまま返すようにし、描画中に間違った番号が例外を起こさないようにする。これだけだと、新しいレーンの点はまだ届いていないので、届くまでの数フレームはレーンが空になる (追加や削除の後と同じ)。点滅を避けたいなら、`peekLanePoints(pending.laneId)` で古いレーンの点をドラッグと一緒に描き、`points?.id === lane.laneId` まで続ける。これが報告の第一案で、プロトコルの変更も要らない。マウントされたレーンで、移動の後に新しい `laneId` が来る場合のテストを足す (上の形)。フェイクのブリッジでは `moveAutoPoints` が新しいレーンの id を与え、シーンが実物と同じ道を通るようにする。

## Fix

Not committed yet (the lead commits).

- `ui/src/components/automation/AutoLane.svelte`: 残す見た目は、それを作った laneId に結びつけられる。答えの新しい laneId の点がまだないとき、ドラッグは作られた元の点 (`peekLanePoints(pending.laneId)`) の上に描かれ、残す見た目は新しい点が届いたとき (または同じ laneId のときは rev に届いたとき) に終わる。保持中のドラッグは laneId を覚える (`dragLane`)。レーンが別の id になったら描かれず、次の移動でキャンセルされ、離しても何も送らない。
- `ui/src/components/automation/autoGeom.ts`: `dragPoints` と `draggedPoints` は、番号が `pts` の中にないとき入力をそのまま返すので、間違った番号が描画中に例外を起こさない。
- `ui/src/bridge/fake.ts`: `moveAutoPoints` は点を新しい laneId の下で動かし、`auto::move_points` と同じく選択する (フェイクのシーンも実物と同じ道を通るようになった)。

Tests: `ui/tests/automation.lane-drag.test.ts` — 「離した後、動かした見た目は、新しいレーンの点が届くまで古い点に残り、例外も二重の移動もない」、「ドラッグ中の点が、レーンが新しい id になった (取り消し、AI のリンク) ときは、ドラッグを手放す: 例外なし、何も送らない」(どちらも前は `TypeError … reading 'tick'` を投げていた)、「移動した点を新しい laneId の下に置き、選ぶ。…」(フェイク用)。
