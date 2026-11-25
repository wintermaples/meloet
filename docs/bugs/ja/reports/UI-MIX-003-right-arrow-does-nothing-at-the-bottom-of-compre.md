---
id: UI-MIX-003
title: コンプレッサーのアタック、ディレイの間隔、リミッターのもどるまでの最小値で、→ キーが何もしない
area: UI-MIX
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-MIX-003: コンプレッサーのアタック、ディレイの間隔、リミッターのもどるまでの最小値で、→ キーが何もしない

## Where

ui/src/components/params/paramRows.ts:103-113 (`keyStep`: `valueAt(p, fractionOf(p, value) + dir / 20)`)
ui/src/components/params/paramRows.ts:13-35 (`decimals`、`roundTo`)

## What happens

対数目盛の 3 つの行で、Shift を押さない → は、最小の値では同じ値を返すため、キーは何もしません。対象は、コンプレッサーの「おさえるまで」（アタック、0.1 ms）、ディレイの「間隔」（ms、同期オフ、1 ms）、リミッターの「もどるまで」（リリース、1 ms）です。範囲の下端から目盛 1 つ分（20 分の 1 の幅）は、×1.41 または ×1.46 になります（0.141 ms、1.41 ms、1.46 ms）。そして、表示する桁（それぞれ 1、0、0 桁）に丸めると、最小の値に戻ります。`change()` は `v === shown` を見て、何も送りません。EFFECT_PARAMS の連続パラメータをすべて、小さな python のスクリプト（同じ計算）で確かめました。範囲の中で矢印キーが動かないのは、この 3 つの値だけです。

## What should happen

spec 4「パラメータで: 値を動かす / 細かく動かす」（← →、Shift ← →）: 押すたびに値が動きます。ほかのすべてのパラメータの、すべての値は → で動きます。

## How to trigger

コードを読んで、python で fractionOf / valueAt / roundTo を写したものを使って確かめました。アプリでは実行していません。挿入に組み込みのリミッターを置き、「もどるまで」を選び、← を押して 1 ms と表示されるまで下げ、それから → を押します。値は 1 ms のままです。Shift → なら 2 ms に動きます。

## Cause

keyStep は、バーの一部の分だけ動かしてから、表示する桁に丸めます。これらの範囲の下端では、動く量が、最後に表示される桁の 1 単位より小さくなります。

## Suggested fix

keyStep で、動かした値が丸めると同じ値に戻るとき、代わりに最後に表示される桁の 1 単位だけ動かします（Shift のように）。例: `const next = valueAt(...); return next !== roundTo(p, value) ? next : roundTo(p, value + dir / 10 ** decimals(p));`。プロトコルの変更はなく、新しい文言もありません。

## Verification

実行して確かめました（一時の作業ツリー。残していません）。`EFFECT_PARAMS` の連続パラメータすべてについて、`keyStep(p, min, +1, false)` と `keyStep(p, max, −1, false)` を開始の値と比べました。動かなかったのは、ちょうど 3 つでした: `compressor[2]`（ms、0.1）、`delay[1]`（ms、1）、`limiter[2]`（ms、1）。報告の通りです。（確かめたのは各範囲の両端だけです。範囲の内側の値で動かないものがないという報告の主張は、もう一度は確かめていません。）

修正案（提案どおり）: `ui/src/components/params/paramRows.ts` の `keyStep` で、動かした値が `roundTo(p, value)` と同じになったとき、代わりに最後に表示される桁の 1 単位だけ動かします。`paramRows` のテストに 3 つのケースを加えます。

## Fix

`ui/src/components/params/paramRows.ts` の `keyStep`: バーの 20 分の 1 を動かした値が同じ値に丸め戻されるとき、最後に表示される桁の 1 単位だけ動かします。テスト: `ui/tests/fix-ui-values.test.ts` の「UI-MIX-003: → moves at the bottom of log-scale rows」。
