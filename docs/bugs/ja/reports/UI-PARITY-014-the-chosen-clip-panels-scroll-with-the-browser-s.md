---
id: UI-PARITY-014
title: 選んだクリップのパネルはブラウザ自身のスクロールバーで動き、パラメータ欄の他の一覧は meloet のものを使う
area: UI-PARITY
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-014: 選んだクリップのパネルはブラウザ自身のスクロールバーで動き、パラメータ欄の他の一覧は meloet のものを使う

## Where

ui/src/components/params/ClipPanel.svelte:276 (グループを囲む `<div class="list">`) と :466-469 (`.list { min-height: 0; overflow-y: auto; }`。スクロールバーを隠す指定はない)
ui/src/components/params/ClipsPanel.svelte:117 と :186-189 (同じ)
正しく作られている仲間: ui/src/components/params/BuiltinList.svelte:129 と params/PluginList.svelte:210 (`<ScrollArea>`。lib/ScrollArea.svelte:120-128 がブラウザのバーを隠し、仕様書 2 の自前のバーを描く)

## What happens

選んだオーディオのクリップのパネル(グループ「クリップ」、「のび縮み ・ 音の高さ」(「音程を寄せる」とその音階の行を含む)) や、複数クリップのパネルが右はしより高いとき(窓が低い、または下の帯が開いているとき)、WebKitGTK 自身のスクロールバーで動く。それは GTK の見た目と色で、テーマのものではない。同じパネルにある組み込みエフェクトとプラグインのパラメータの一覧は、meloet の幅 8 のバー (gray.3 の地、gray.6 のつまみ) で動く。

## What should happen

仕様書 2「スクロールバー」: 「ブラウザや OS のバーは使わない。一覧、ダイアログの中身、出口えらび、パラメータ、棚で同じものを使う」。同じパネルのパラメータの一覧は、すでに `ScrollArea` を使っている。

## How to trigger

コードから読んだもので、実行はしていない。
1. 窓を 1024 × 640 にし、ミキサーを下の帯として開き、右はしを短くする。
2. パネルが前にある状態で、オーディオのクリップを一つ選ぶ(すべてのグループが出るように、のび縮みを入れる)。
3. グループが入りきらない。`.list` にネイティブの縦スクロールバーがつく。

## Cause

ClipPanel と ClipsPanel は、グループを `ScrollArea` で囲まず、ふつうの `div` を `overflow-y: auto` でスクロールさせている。

## Suggested fix

`.list` の中身を `<ScrollArea>` で囲み (BuiltinList と同じ)、`.list` から `overflow-y: auto` を外す。プロトコルや文面の変更はない。見た目は仕様書 2 のバーになる。

## Verification

コードを読んで確認したもので、実行はしていない(写真も撮っていない)。`ClipPanel.svelte:466-469` と `ClipsPanel.svelte:182-185` は、ふつうの `div` (`.list { min-height: 0; overflow-y: auto; }`) でスクロールさせ、ブラウザのバーを隠すものはない (`scrollbar-width: none` / `::-webkit-scrollbar` は `lib/ScrollArea.svelte` と `mixer/MixerStrips.svelte` にしか出てこない)。仕様書 2 の 94 行目、そのままの文: 「スクロールバー | 幅 8。地は gray.3、左に 1px の line.thin の線、つまみは gray.6。角 0。ブラウザや OS のバーは使わない。一覧、ダイアログの中身、出口えらび、パラメータ、棚で同じものを使う」。

修正計画 (提案どおり): `ui/src/components/params/ClipPanel.svelte` と `ClipsPanel.svelte` のグループを、`BuiltinList.svelte:129` のように `<ScrollArea>` で囲み、`.list` から `overflow-y: auto` を外す。低い窓で `tools/design/shoot_ui.py` を使い、バーが幅 8 のものであること、行の幅が保たれることを確かめる(表示されると、バーは 8 px を取る)。

## Fix

`ui/src/components/params/ClipPanel.svelte` と `ClipsPanel.svelte`: グループは `<ScrollArea>` の中に入った(BuiltinList と同じ)。`overflow-y: auto` の `.list` の規則は消えた。テストはなし(見た目だけ)。`multiClip` の写真 (shoot_ui、1280x800、100/125) は、入りきるときのパネルが前と同じであることを示す。右はしより高いパネルの写真は撮っていない。
