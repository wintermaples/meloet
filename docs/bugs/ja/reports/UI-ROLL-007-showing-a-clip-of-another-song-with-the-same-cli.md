---
id: UI-ROLL-007
title: 同じクリップ id を持つ別の曲のクリップを表示すると、前の拡大、スクロール、行が残る
area: UI-ROLL
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-ROLL-007: 同じクリップ id を持つ別の曲のクリップを表示すると、前の拡大、スクロール、行が残る

## Where

- `ui/src/components/pianoroll/Roll.svelte:489-503` — 開いたときの合わせ(拡大、クリップの始めまでのスクロール、そのあとノートまでの行の合わせ)は、`if (prView.shownClip !== clip.id)` のときだけ動く。
- `ui/src/components/pianoroll/view.svelte.ts:40` — `prView.shownClip` は、アプリが動いている間ずっと残るモジュールの状態である。別の曲が来ても、何も消さない(`grep shownClip` で見つかるのは、この二か所だけ)。
- `crates/meloet-model/src/mixer.rs:354` — `NEW_SONG_NEXT_ID = 4`。新しい曲はどれも同じ id を同じ順に配るので、id は曲ごとである。
- 比べる: `ui/src/components/pianoroll/shown.svelte.ts:283-298` と `ui/src/components/pianoroll/ghosts.ts:25-28`。どちらも `song.serial` で曲を分けている。

## What happens

新しい曲(Mod N)や、別の曲を開いたあと、ピアノロールに表示するクリップが、前の曲で最後に表示したクリップと同じ id だと、「もう開いている」と扱われる。拡大、横のスクロール、一番上の行は、前の曲のままになる。新しいクリップのノートは表示の外に出ることがあり、ピアノロールは空に見えたり、クリップが合わない拡大率で表示されたりする。新しい曲は id を 4 から配り始めるので、同じように作った二つの新しい曲で最初に録音したクリップは、同じ id になることが多い。

## What should happen

仕様 3: 「ピアノロールが開くと、そのクリップの始まりまでスクロールする(ループが入っていて、クリップと重なるときは、ループの始まりまで)」。アーキテクチャ 16、「表示する範囲」: 「開くとき、クリップに合う一番大きい段階を使い、クリップの始まりまでスクロールし、縦方向はノートのある所まで合わせる」。表示するテイク(`shown.svelte.ts`)とゴーストの一覧は、別の `song.serial` を別の曲として扱う。

## How to trigger

コードを読んで確かめたもので、実行はしていない。

1. 新しい曲。鍵盤の高い所で短いフレーズを録音し、そのクリップをピアノロールで開いて、右へ大きくスクロールする。
2. Mod N。同じように、鍵盤の低い所で短いフレーズを録音する(同じクリップ id になる)。そのクリップを押す。
3. ピアノロールは、前の曲のスクロール、拡大、行のままになる。新しいノートは見えない。

## Cause

比べるのはクリップ id だけで、クリップ id は曲をまたいで一意ではない。`song.serial` は比べられていない。

## Suggested fix

表示中のクリップと一緒に曲の serial を保つ(例: `prView.shownClip = { id, serial }` として両方を比べる)か、`song.serial` が変わったとき `prView.shownClip = null` にする。プロトコルは変えない。

## Verification

読んで確かめた(実行はしていない)。`Roll.svelte:489` は `if (prView.shownClip !== clip.id)` のときだけ合わせる。`prView.shownClip`(`view.svelte.ts:40`)はモジュールの状態で、ほかでは書かれていない(検索結果: この二か所だけ)。そのため、曲をまたいで残る。id は曲ごとで、新しい曲は `NEW_SONG_NEXT_ID = 4` から始まる(`crates/meloet-model/src/mixer.rs:354`、`song.rs:409`)。そのため、同じように作った二つの新しい曲の最初のクリップは同じ id になる。同じファイルを開き直しても、同じ id になる。`shown.svelte.ts:54-74` は、`song.serial` で曲を分けている。

深刻度は low に下げた。ピアノロールが一度だけ、誤った拡大とスクロールを保つだけで、何も失われない。一度拡大かスクロールをすれば直る。READMEの言い方では小さな見た目の不具合である。

修正計画(提案どおり): `ui/src/components/pianoroll/Roll.svelte` で、曲も比べる。`view.svelte.ts` で `prView.shownClip` と一緒に曲の `song.serial` を保ち(二つ目のフィールド)、クリップ id か曲の serial のどちらかが違えば、合わせ直す。テスト: クリップ id 5、serial 1 を表示してマウントする。そのあと serial 2 で同じクリップ id の状態にすると、スクロールがクリップの始まりに再び合わせられる。

## Fix

プロトコルは変えない。

- `ui/src/components/pianoroll/view.svelte.ts`: `prView.shownSerial`(`prView.shownClip` の曲の serial)を追加する。
- `ui/src/components/pianoroll/Roll.svelte`: 開いたときの合わせは、クリップ id か曲の serial が違うときに動く。
- テスト: `ui/tests/pianoroll.hold-mount.test.ts` の「fits again (zoom and scroll to the clip); the same song's clip keeps the view」。
