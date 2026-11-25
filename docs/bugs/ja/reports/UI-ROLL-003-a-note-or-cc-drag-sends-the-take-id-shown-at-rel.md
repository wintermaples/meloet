---
id: UI-ROLL-003
title: ノートや CC のドラッグが、押したときに選んだインデックスと、離したときに表示されているテイクの id を送る
area: UI-ROLL
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-ROLL-003: ノートや CC のドラッグが、押したときに選んだインデックスと、離したときに表示されているテイクの id を送る

## Where

- `ui/src/components/pianoroll/drag.ts:191-255` — `down` は、`notes`、`grab`、`hits` を、押したときに表示されていたテイクのインデックスとして選ぶ。ドラッグは、そのテイクの id を保持しない。
- `ui/src/components/pianoroll/drag.ts:375-381` — `up` は `this.takeId()`（離したときに表示されているテイク）を読み、`finish` に渡す。
- `ui/src/components/pianoroll/drag.ts:452-473` — `finish` は、その id を押したときのインデックスと合わせて、`moveNotes`、`resizeNotes`、`deleteNotes`、`changeVelocity`、`selectNotes` に入れる。
- `ui/src/components/pianoroll/cc.ts:352-357` — `LaneMouse.up` は、`moveControls`、`deleteControls`、`selectControls` で同じことをする。
- `ui/src/components/pianoroll/Roll.svelte:280` と `CcLanes.svelte:85` — `shown()` は、いま表示されているテイクを返す。テイクが変わったときに、ドラッグを終えたり印を付けたりするものはない。

## What happens

マウスのボタンを押したままでテイクが変わると、離したときに、新しいテイクの id が、古いテイクのインデックスと一緒に送られます。コアは受け入れます。id は、いま表示されているテイクのものだからです。そして、新しいテイクで同じインデックスのノートを、なんでも動かし、長さを変え、ベロシティを変え、消します。選択も間違ったノートになります。ボタンを押したままでもテイクは、キーで変わることがあります（キーの処理は、ドラッグを待ちません。Mod Z、Mod Shift Z、Delete、↑ ↓ ← →、Q、T）。また、AI の連携からの編集で変わることもあります。さらに、押す前に終わった編集の新しいテイク（少し前に離したドラッグ、↑ ↓）が、押したあとに届くことでも変わります。

## What should happen

architecture 16「ノートの順番と、ノートの指し方」: 「インテントは、テイクの id とそのテイクのなかのインデックスで指す。id が表示中のテイクでなければ、Rust は何もしない（たとえば、ドラッグを離した直後にキーを押した場合に、古いインデックスが別のノートを編集しないように）」。この守りが効くのは、送った id が、インデックスを読んだテイクの id である場合だけです。

## How to trigger

コードを読んで確かめました。実行はしていません。
1. ノート A、B、C（インデックス 0、1、2）があるクリップで、A の前に新しいノート X を描きます（取り消しの手順 1 つ。テイクは X、A、B、C になり、B はインデックス 2 になります）。
2. B を押して、右へドラッグし始めます。ボタンは押したままにします。
3. Mod Z を押します。テイクは、前の id の A、B、C に戻ります（インデックス 2 は C です）。
4. 離すと、`moveNotes { take: <earlier id>, notes: [2] }` が送られ、B ではなく C が動きます。消しゴムの道具で同じ手順をすると、C が消えます。

## Cause

`RollDrag.up()` と `LaneMouse.up()` は、テイクの id を、押したときではなく、離したときの `input.shown()` から取っています。ドラッグのインデックスは、押したときのテイクのものです。

## Suggested fix

ドラッグの中に、押したときのテイクの id を保持し（`Drag` と `LaneDrag` に `take` フィールドを置く）、`finish` と `finishLane` でその id を使います。そうすると、そのテイクがもうないドラッグは、コアが何もしません。任意で、`n.take.id` がドラッグ中に変わったら、ドラッグを取り消す（Esc と同じ）こともできます。プロトコルは変えません。

## Verification

コードを読んで確かめました（実行はしていません）。`RollDrag.down` は、押したときに表示されていたテイクのインデックスを保持し、テイクの id は保持しません。`up()` は離したときに `this.takeId()` を読み（`drag.ts:375-381`）、`finish(d, take)` が、その id を古いインデックスと合わせます（452-473）。`LaneMouse.up` も同じです（`cc.ts:352-357`）。表示されているテイクが変わったときにドラッグを取り消すものはありません（`Roll.svelte` と `CcLanes.svelte` を検索すると、`cancel()` は Esc と `pointercancel` からだけ呼ばれます）。また `handleKeydown`（`keys/attach.ts`）は、保持されているドラッグを見ないので、ドラッグ中の Mod Z もコアに届きます。Rust の守り（architecture 16「id が表示中のテイクでなければ、Rust は何もしない」）は、送られた id が新しいものなので通ってしまいます。

修正案（提案どおり）: `ui/src/components/pianoroll/drag.ts` で、ドラッグが `down` で押したときのテイクの id を保持し、`up` と `finish` で使います。`cc.ts` の `LaneDrag` も同じです。コアは、テイクがもうないドラッグには何もしません（描かれた見た目は自然に戻ります）。また、`n.take.id` が変わったら保持中のドラッグを取り消します（そうしないと、動いている絵が、新しいテイクのノートに古いインデックスで描かれてしまい、UI-ARRANGE-001 と同じ種類の不具合になります）。テストは `ui/tests/pianoroll*.test.ts` に置きます。押し、表示テイクを変え、離すと、インテントは押したときの id を運ぶ、という内容です。

## Fix

プロトコルの変更はありません。

- `ui/src/components/pianoroll/drag.ts`: `RollDrag` は、押したときに表示されていたテイクの id（`pressTake`）を保持し、離したときに送る。`dropIfTakeChanged()` は、別のテイクが表示されたら保持中のドラッグを取り消す。
- `ui/src/components/pianoroll/cc.ts`: `LaneMouse` も同じ。
- `ui/src/components/pianoroll/Roll.svelte`、`ui/src/components/pianoroll/CcLanes.svelte`: 表示テイクの id の effect が `dropIfTakeChanged()` を呼ぶ。
- テスト: `ui/tests/pianoroll.drag.test.ts` の「a drag keeps the take of the press (UI-ROLL-003)」（3 件）。`ui/tests/pianoroll.cc.test.ts` の「CC drags: the take of the press, and while recording (UI-ROLL-003, UI-ROLL-008)」。`ui/tests/pianoroll.hold-mount.test.ts` の「a drag of the take shown at the press (UI-ROLL-003)」。
