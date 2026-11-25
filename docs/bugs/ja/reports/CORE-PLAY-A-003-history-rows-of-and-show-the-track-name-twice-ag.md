---
id: CORE-PLAY-A-003
title: 「アルペジオを変える」と「音のつまみを変える」の履歴の行に、トラック名が二度出る。仕様 3 (94-6) に反する
area: CORE-PLAY-A
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# CORE-PLAY-A-003: 「アルペジオを変える」と「音のつまみを変える」の履歴の行に、トラック名が二度出る。仕様 3 (94-6) に反する

## Where

- crates/meloet-core/src/play/history.rs:1136-1142 (`StepKind::SetArp`: 何を `StepWhat::Track { track }`、場所は `on_track(track)`)
- crates/meloet-core/src/play/history.rs:1143-1160 (`StepKind::SetSoundKnob`: 何を `StepWhat::SoundKnob { track, .. }`、場所は `on_track(track)`)
- ui/src/components/history/stepText.ts:199-203 (`rowWhatText` は、`soundReplaced` の場合だけトラックを外す)
- ui/src/bridge/fakeWave3.ts:27 (偽物のデータも同じ行を作る)

## What happens

取り消しの履歴の帯で、「アルペジオを変える」の行は「「アルペジオを変える」・ 1 ピアノ」と読め、右の欄にも「1 ピアノ」がもう一度出ます。「音のつまみを変える」の行は「「音のつまみを変える」・ 1 ピアノ ・ 明るさ 50 → 25 %」と読め、右の欄にも「1 ピアノ」がもう一度出ます。

## What should happen

仕様 3 (94-6、3-bands.md 272行目): 「行の右の欄にトラック名が出る手については、「（何を）」にトラック名を含めない」。仕様 7 の readings の396行目は、これを「音源を入れかえる」にも当てはめています (窓の行だけは、オーナーの決めで二度の言い方を残す)。隣の `soundReplaced` の行は、すでにこれに従っています。場所がトラックのとき、`rowWhatText` は「グランドピアノ → オルガン」だけを書きます。

## How to trigger

コードを読んで確かめたもので、実行はしていません。トラック1のアルペジオを変える (「アルペジオ ▾」)、または内蔵の音のつまみをドラッグする。取り消しの履歴の帯を開き、いちばん新しい行を見る。

## Cause

どちらの手の種類も、トラックを「（何を）」に入れ、さらに場所としてもトラックを示します。`rowWhatText` (stepText.ts 201) は `what.kind === "soundReplaced"` のときだけトラックを外します。`track` (SetArp) と `soundKnob` は、トラックのラベルを付けたまま `stepWhatText` を通ります。

## Suggested fix

画面側で、`soundReplaced` と同じように行います。`rowWhatText` で、`row.place?.kind === "track"` かつ `row.what.kind === "track"` (同じトラック) のとき、`history.rowBare` を返す。`soundKnob` では、トラックを付けない形 (「明るさ 50 → 25 %」) にする。状態行は右の欄がないので、トラックを残せます。トラックを付けない `soundKnob` の形には、`ja.ts` / `en.ts` の文面のキーが必要です (リードが決めること。または既存のキーを使う)。SetArp の半分は、キーが要りません。別案として、Rust で `SetArp` に `StepWhat::None` を返させる方法もありますが、その場合は取り消しの状態行もトラック名を失います。

## Verification

コードを読んで確かめたもので、テストは実行していません。`play/history.rs:1136-1160` は、場所を `on_track(track)` とした `StepWhat::Track` (SetArp) と `StepWhat::SoundKnob` を作ります。`rowWhatText` (`ui/src/components/history/stepText.ts:199-203`) は `soundReplaced` のときだけトラックを外すため、どちらの行も「（何を）」にトラック名を書き、右の欄にもう一度書きます。仕様 3 (94-6): 「行の右の欄にトラック名が出る手については、「（何を）」にトラック名を含めない」。`7-readings.md` の396行目は、右の欄が「アレンジ」か「ミキサー」の行だけ名前を残すとしています。重大度 `low` は変えません。

文面については、新しいテキストのキーは要りません。そのため `confirmed` にして、`needs-lead` にはしません。場所をトラックとした「アルペジオを変える」は、トラックを外した行 (`history.rowBare`) になります。つまみの行からトラックを外した形は「明るさ 50 → 25 %」で、既存のキーで言えます。`history.changeAt` (`{place} {from} → {to}`、仕様 3) の `place` をつまみの名前、`from` を数値、`to` を `param.value.percent` (`{value} %`、仕様 3) にします。どちらのキーも `ja.ts` と `en.ts` にあります。

修正計画 (画面だけ): `ui/src/components/history/stepText.ts` の `rowWhatText`: `row.place?.kind === "track"` のとき、`what.kind === "track"` なら `null` を返す (トラックを外した行)。`what.kind === "soundKnob"` なら、上の `history.changeAt` の形を返す。状態行は、トラックを付けたまま `stepWhatText` を使う。`ui/src/bridge/fakeWave3.ts:27` もこれに合わせる。テストは、履歴の文面のテストで `soundReplaced` の場合の隣に置く。

## Fix

`ui/src/components/history/stepText.ts` の `rowWhatText`: 場所がトラックのとき、同じトラックの `track` の「（何を）」は、トラックを外した行になり、`soundKnob` は、つまみの名前と `param.value.percent` を使った `history.changeAt` になります (「明るさ 50 → 25 %」)。状態行は `stepWhatText` のままです。偽物 (`fakeWave3.ts`) は Rust と同じ行を作るので、変更は要りませんでした。テスト: `ui/tests/fix-ui-values.test.ts` の「CORE-PLAY-A-003: rows whose right column is the track do not name it again」。
