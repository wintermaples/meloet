---
id: UI-PARITY-007
title: 読めない Enter のあと、ミキサー・メトロノーム・パーセントの入力の箱が、フォーカスが離れても閉じなくなる
area: UI-PARITY
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# UI-PARITY-007: 読めない Enter のあと、ミキサー・メトロノーム・パーセントの入力の箱が、フォーカスが離れても閉じなくなる

## Where

ui/src/components/params/ValueField.svelte:27-31 (`done = true` は、`onsubmit` の前に、`onsubmit` が何をしても設定される)
ui/src/components/params/ValueField.svelte:42-44 (`onblur` は `!done` のときだけ取り消す)
ui/src/components/mixer/control.svelte.ts:121-124 (`submit` は、読めない文字に対して false を返し、`typing` をオンのまま残す。フェーダー、パン、両方のセンドで)
ui/src/components/TrackHeads.svelte:48-50 (トラックの見出しの dB: 早く戻り、箱は残る)
ui/src/components/MetronomeWindow.svelte:153-156 (メトロノームの音量: 「読めない文字は箱を開いたままにする」)
ui/src/components/pianoroll/PercentSlider.svelte:83-85 (クオンタイズの強さとスウィング: 早く戻り、箱は残る)

## What happens

フェーダー、パン、センド、トラックの見出しの dB、メトロノームの音量、クオンタイズの強さ・スウィングの値の箱で、読めない文字 (たとえば「あ」) を打って Enter を押すと、箱は開いたままになる。これは意図どおりである (7-readings A-18 「読めない文字は箱を開いたままにする」)。そこから先、箱はフォーカスを離しても閉じない。ほかの場所を押すとフォーカスは移るが、箱は打ちこんだ文字を載せたまま画面に残り、その下の数字は隠れたまま。出る方法は、箱をもう一度押して Esc を使うか、読める Enter を使うしかない。Enter を押さないままの場合、同じ箱は外側を押すと閉じる。

## What should happen

ValueField の見出しには「箱の外を押して離れても止まる (Enter だけが決める)」とある。その兄弟の箱は、どの場合にもフォーカスが離れると閉じる。NumberField.svelte:85 (`onblur={() => close(false)}`)、timeline/ValueInput.svelte:66 (`onblur={close}`)、pianoroll/NoteFieldInput.svelte:65 (`onblur={oncancel}`) は、読めない Enter のあとでも、「完了」の印を設定しないので閉じる。

## How to trigger

実行せず、コードを読んで確かめた。
1. ミキサーを開き、フェーダーの dB の文字をダブルクリックする (または、フェーダーへ Tab で移り、Enter を押す)。
2. 「あ」を打って Enter を押す: `ValueField.onkeydown` は `done = true` を設定し、`onsubmit` を呼ぶ。`parseDb("あ")` は undefined。`submit` は false を返し、`typing` はオンのまま。
3. ミキサーの別の場所を押す: `onblur` が走る。`done` は true なので、`oncancel` は呼ばれない。箱は残る。
パンの文字、センドの文字、トラックの見出しの dB、メトロノームの音量の数字、クオンタイズの強さ・スウィングの値でも同じ。

## Cause

`done` は、決まった Enter のあとに続く blur が、取り消しまでしてしまうのを防ぐためにある。しかし、親が文字を受け取ったかどうかを言う前に設定される。そして、親が箱を開いたままにしたときに戻されることがない。送信のたびに必ず閉じる親 (ParamRow、ClipPanel、ClipsPanel、SettingsDialog の遅延) は、この問題に当たらない。上の五つの親は、読めない文字に対して箱を開いたままにするので、true のまま残った done を持つ同じ箱がマウントされ続ける。そのため、あとの blur では何も閉じない。

## Suggested fix

`onsubmit` に、箱を閉じたかどうかを返させ、true のときだけ `done` を設定する (ミキサーの `submit` は、すでに真偽値を返している。ほかの親も返すようにする)。または、箱がマウントされたままの `onsubmit` のあとで `done = false` に戻す。プロトコル、文言、見た目の変更はなし。

## Verification

テストを走らせず、読んで確かめた。`ValueField.svelte:27-31` は、`onsubmit` を呼ぶ前に `done = true` を設定する。`onblur` (42-44) は `!done` のときだけ取り消す。読めない文字に対して箱を開いたままにする親 (`mixControl.submit` は false を返し `typing` をオンのまま残す。`control.svelte.ts:118-125`。`TrackHeads.svelte:48-50`。`MetronomeWindow.svelte:153-156`。`PercentSlider.svelte:83-85`。7-readings A-18「読めない文字は箱を開いたままにする」) は、同じ `ValueField` をマウントしたままにし、`done` は true に詰まる。そのため、あとの blur では何も閉じない。`ValueField.svelte` の見出しは、箱の外を押すと止まると書いている。

重大度は low に下げた。最初に読めない Enter が必要で、Esc か読める Enter で箱は今も閉じる。曲には何も触れない。

修正案: `ui/src/components/params/ValueField.svelte` で、`onsubmit` が戻ったあとに、箱がまだあれば (読めない文字の場合) `done` を false に戻す (親を変えない最小の変更)。または、`onsubmit` に閉じたかどうかを返させる。テスト: 読めない Enter のあと blur → `oncancel` が呼ばれる。

## Fix

`ui/src/components/params/ValueField.svelte`: Enter のあと、次の tick でも箱がまだあるとき (読めない文字のとき)、`done` を false に戻す。そのため、箱を離すと止まる。親は変えていない。テスト: `ui/tests/fix-ui-values.test.ts` の「UI-PARITY-007: after an Enter that could not be read, leaving the box closes it」。
