---
id: UI-ARRANGE-010
title: 拍子の旗の箱: 読めない下の数字が、上の数字として知らされる
area: UI-ARRANGE
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-010: 拍子の旗の箱: 読めない下の数字が、上の数字として知らされる

## Where

ui/src/components/timeline/TempoBand.svelte:302 (`reject={() => send({ type: "rejectValue", field: "meterNum", kept: String(editFlag.num), range: null })}`)

## What happens

開いている「テンポ ・ 拍子の変更」の帯で、拍子の旗の箱 (旗を二回押す、または「拍子を打ちこむ…」) は、読めないすべての文字に対して、`field: "meterNum"` と上の数字を付けた `rejectValue` を送る。上の数字は正しく、下だけが読めない (「4/x」、「4/」) とき、状態行は「拍子の上は数字で打ちます。もとの 4 のままです。」と言い、上の数字のせいにする。`ValueInput` は入力された文字を `reject` に渡すが、帯はそれを無視する。

## What should happen

再生まわりの拍子の箱は、どちらの半分が悪いかを伝える。`ui/src/components/Transport.svelte:303-306` の `rejectMeter(draft)` は `meterFieldOf(draft)` (上が 1〜12 なら下) を使い、その半分の数字を残す。旗の箱も、同じ文字には同じことを伝えるべきである (仕様書 6「数字として読めない」: 「{field}は数字で打ちます。もとの {kept} のままです。」)。

## How to trigger

コードを読んで分かったこと。実行はしていない。帯を開き、4/4 の旗を二回押して「4/x」と入力し、Enter を押す。状態行は「拍子の上」と言う。同じ文字を再生まわりの拍子の箱に入れると、「拍子の下」と言う。

## Cause

`reject` のコールバックは、入力された文字から欄を選ぶ代わりに、`field: "meterNum"` を固定で書かれている。

## Suggested fix

再生まわりと同じく、入力された文字を使う: `reject={(draft) => { const field = meterFieldOf(draft); send({ type: "rejectValue", field, kept: String(field === "meterNum" ? editFlag.num : editFlag.den), range: null }); }}` (`meterFieldOf` は `components/time.ts` にある)。プロトコルや文言は変えない。

## Verification

コードを読んで確認した (実行はしていない)。`TempoBand.svelte:302` は固定の `field: "meterNum"` を送り、下書きを無視する。`ValueInput.svelte:39` は入力された文字を渡している (`reject(draft)`)。再生まわりの箱は `meterFieldOf(draft)` を使う (`Transport.svelte:303-306`、`time.ts:88-92`: 上が 1〜12 と読めれば下)。両方の箱は同じ `readMeter` で読む。

また、UI-PARITY-004 としても報告されていた (同じ行、同じ修正。その報告はこれに統合して削除された)。そこから: `timeline/ValueInput.svelte` の冒頭は「見た目と決まりは、再生まわりの数字の箱 (NumberField.svelte) と同じ」と書いており、仕様書 6「数字として読めない」は、読めなかった欄とその元の値を名指す。

修正案 (提案どおり): `ui/src/components/timeline/TempoBand.svelte:302` で下書きを使う: `reject={(draft) => { const field = meterFieldOf(draft); send({ type: "rejectValue", field, kept: String(field === "meterNum" ? editFlag.num : editFlag.den), range: null }); }}`。`ui/tests/round6.timeline.test.ts` に場合を 1 つ足す (旗で「4/x」→ `field: "meterDen"`)。プロトコルや文言は変えない。

## Fix

まだ commit していない (commit は lead が行う)。

- `ui/src/components/timeline/TempoBand.svelte`: 旗の箱の `reject` は入力された文字を使う。`meterFieldOf(draft)` が半分を選び、その半分の古い数字を残す (再生まわりの箱と同じ)。旧 UI-PARITY-004 も対象になる。

テスト: `ui/tests/round6.timeline.test.ts` — 「旗に入力された読めない下の数字は、再生まわりの箱と同じく下の数字として知らされる (仕様書 6)」。
