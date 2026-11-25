---
id: UI-ARRANGE-012
title: 新しい歌のトラックの入る枠が、最後のトラックの行のすぐ下に描かれ、その下のレーンや帯に重なる
area: UI-ARRANGE
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-012: 新しい歌のトラックの入る枠が、最後のトラックの行のすぐ下に描かれ、その下のレーンや帯に重なる

## Where

ui/src/components/arrange/audioClipGeom.ts:179-183 (`dropFrameBox`: 新しいトラックのとき `top = rowTop(d, rows - 1) + rowHeight(d, rows - 1)`)
ui/src/components/arrange/DropFrame.svelte:40 (`dropFrameBox(view, dims, target, tracks.length, 2 * perBar)`)

## What happens

音声ファイルをドラッグしている間、ドロップすると新しい歌のトラックができるとき(キーボードのトラックの上、空いている所の上、または「別々のトラックに」)、gray.7 の枠は「最後の行のすぐ下」に置かれる。そのために、最後のトラックの「行」の下端を使う。ところが最後のトラックに開いたオートメーションのレーンや「＋ 帯を足す ▾」の行、または開いた「いいとこ取り」の帯があると、枠はそれらの上に描かれる。新しい歌のトラックは、本来それらの下に現れる(新しい歌のトラックは最後に追加される。`add_vocal_track(&new, None, …)`、`crates/meloet-model/src/audio/clips.rs:236`)。

## What should happen

仕様 3(29-A): 「持ってきている間、入る所に gray.7 の枠」。枠は、ファイルがどこに入るかを示すべきである。`dropFrameBox` のコメントは「新しいトラックは、最後の行のすぐ下」と書いている。すべてのトラックのブロックの下端は、`arrangeRows`(`automation/autoGeom.ts:96`)の `rows.tracksHeight` である。

## How to trigger

コードを読んで確かめたもので、実行はしていない。最後のトラックにボリュームのレーンを開く。そのあとファイルマネージャーから .wav ファイルをキーボードのトラックの上にドラッグする。枠は、最後のトラックのボリュームのレーンの上に描かれる。

## Cause

`dropFrameBox` が知っているのは、行の上端と高さ(`LaneDims`)だけで、その下にあるレーン、帯、マスターの行の高さは知らない。

## Suggested fix

トラックのブロックの下端(`rows.tracksHeight`、または `rows.bottoms[rows.length - 1]`)を `dropFrameBox` に渡し、新しいトラックの `top` として使う。プロトコルも文言も変えない。

## Verification

読んで確かめた(実行はしていない)。`dropFrameBox`(`arrange/audioClipGeom.ts:179-183`)は、新しいトラックのために `rowTop(d, rows - 1) + rowHeight(d, rows - 1)` を使う。`rowTop` / `rowHeight` はトラックの行だけを扱う(`arrangeGeom.ts:41-44`)。そのため、最後のトラックの開いたレーン、その「＋ 帯を足す ▾」の行、開いた「いいとこ取り」の帯は、その y の下にあり、枠はそれらの上に描かれる。新しい歌のトラックは、それらすべての後ろに追加される。

修正計画(提案どおり): `dropFrameBox` にトラックのブロックの下端(`automation/autoGeom.ts:96` の `arrangeRows` の `rows.tracksHeight`)を渡す。`DropFrame.svelte:40` は、それを prop として `Arrange.svelte` から受け取る。`target.row === rows` のとき、それを `top` として使う。`dropFrameBox` のテストに一件を足す。

## Fix

`ui/src/components/arrange/audioClipGeom.ts` の `dropFrameBox` は、トラックのブロックの下端(`tracksBottom`)を受け取り、新しいトラックの枠をそこに置く。`ui/src/components/arrange/DropFrame.svelte` はそれを渡す。`ui/src/components/Arrange.svelte` は `rows.tracksHeight` を渡す。
テスト: `ui/tests/arrange.audioGeom.test.ts`(`dropFrameBox` の件: 最後のトラックのレーンと「＋ 帯を足す ▾」の行がある場合、枠はそれらの下にある)。
