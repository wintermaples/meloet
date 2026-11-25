---
id: UI-SHELL-008
title: キーの一覧にメトロノームの窓のキーと、「書き出しました」の Enter がない (仕様 4)
area: UI-SHELL
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-SHELL-008: キーの一覧にメトロノームの窓のキーと、「書き出しました」の Enter がない (仕様 4)

## Where

ui/src/keys/keyList.ts:96-118 (`OTHER_ROWS`: keys.ts の外で受け取るキー)
ui/src/components/MetronomeWindow.svelte:136-145, 184-189 (← → で音量を 1 dB 動かす。Space と Enter で「タップ」を押す)
ui/src/keys/attach.ts:534-537 (Enter で「書き出しました」を閉じる)
docs/spec/4-keys.md:81-82 (仕様 4 の 2 つの行。82 行目は UI-PARITY-006 としても報告されている)

## What happens

キーの一覧のダイアログ (ヘルプ ▸ 「キーの一覧」) には、仕様 4 の 2 つのキーの行がない。「メトロノームの窓で: 音量を 1 dB 動かす ／ 「タップ」でテンポを手拍子で — ← → ／ Space Enter」 と、「「書き出しました」を閉じる — Esc ／ Enter」 である (Esc は「ひとつ戻る ／ 閉じる」に含まれているが、Enter は含まれていない)。キーの一覧で「タップ」や「メトロノーム」を検索しても、見つかるのは C のキーだけである。

## What should happen

docs/spec/4-keys.md の表の上の規則: 「アプリに追加したキーは、キーの一覧のダイアログ (`ui/src/keys/keyList.ts`、round 12 G1) にも追加する」。どちらの行も仕様 4 の表にあり、どちらも実装されている。

## How to trigger

実行して確かめたのではなく、コードを読んで確かめた。ヘルプ ▸ 「キーの一覧」を開き、「タップ」と入力する: 行がない。

## Cause

`OTHER_ROWS` には、タイムライン、ミキサー、パラメータ、ドラッグ、箱のキーが入っているが、メトロノームの窓 (round 12 R3) と E9 の Enter は、その後に追加されたため入っていない。docs/progress.md には、一覧を仕様 4 と 1 行ずつ照らし合わせて確かめるものがない、と書かれている。

## Suggested fix

`OTHER_ROWS` に行を追加する: メトロノームの窓は、グループ「play」(← → ／ Space Enter)。「書き出しました」を Enter で閉じるのは、グループ「file」か「play」 (どちらにするかは決める)。ラベルの新しい文言が要る (`keyList.*`) ので、リード・オーナーの決定が要る (既存の `keyList.*` のラベルは、オーナーが承認した `builder:` の文言である)。

## Verification

実際に読んで確かめた (実行はしていない): `OTHER_ROWS` (`ui/src/keys/keyList.ts:96-118`) には、仕様 4 の 82 行目 (「メトロノームの窓で: 音量を 1 dB 動かす ／ 「タップ」でテンポを手拍子で — ← → ／ Space Enter」) の行も、81 行目の Enter (「「書き出しました」を閉じる — Esc ／ Enter」) の行もない。どちらも実装されている (`MetronomeWindow.svelte:136-145, 184-189`、`attach.ts:534-537`)。仕様 4 の 10 行目: 「アプリに追加したキーは、キーの一覧のダイアログにも追加する」。7-readings の G1「ほかのキーの行き先」は、書き出しの出口えらびのキーだけを意図して外している。`docs/progress.md` には、一覧を仕様 4 と照らし合わせるものがない、ということしか書かれていない。

UI-PARITY-006 としても報告された (メトロノームの半分。このレポートにまとめて、そちらは削除した)。

`needs-lead` の理由: 2 つの行には、まだない文言が要る。`ui/src/text/ja.ts` で合いそうなキーを探した。`keyList.*` のラベルは、どれも作られた 1 つの言い回し (`builder:2026-11-23`、7-readings G1 に載っている) である。`row()` は既存のキーを 「 ／ 」 でつなげるが、「音量 ／ タップ」 (`metronome.volume`、`metronome.tap`) では、どこでなのかが分からない。また、「書き出しました」を閉じるということは、どこにも書かれていない。

推奨する文言 (今ある行の形で):
- メトロノームの行、グループ「再生と録音」(`play`、C の「メトロノーム」がある所)、キー `← → ／ Space Enter`:
  ja 「メトロノーム：音量を動かす ／ テンポを手拍子で」、en "Metronome: move the volume / tap the tempo"。言葉の出どころ: `menu.metronome` 「メトロノーム」 / "Metronome"; `metronome.volume` 「音量」 / "Volume"; `keyList.fader` の動詞 「…を動かす」 / "Move …"; `metronome.tapRow` 「テンポを手拍子で」 / "Tap the tempo" (その行のオーナーの文言); `keyList.shelfSample` の 「名前：…」 の形。
- 書き出しの行、グループ「曲とファイル」(`file`)、キー `Esc ／ Enter`:
  ja 「「書き出しました」を閉じる」、en "Close “Exported”"。言葉の出どころ: `exported.title` 「書き出しました」 / "Exported"; `button.close` (仕様 6) と `keyList.back` の 「閉じる」; 仕様 4 の 81 行目そのもの (「「書き出しました」を閉じる」)。

選択肢: (a) この文言で 2 つの行を追加する (オーナーが同意するまでは出どころは `builder:<date>`、7-readings G1 に 1 行書く)。(b) メトロノームの行だけを追加し、7-readings G1 に「ダイアログの Enter のキーは一覧に載せない」と書く (ほかのダイアログの Enter は一覧にない。Esc は「ひとつ戻る ／ 閉じる」に含まれている)。(c) 2 つとも入れず、7-readings G1 にそう書く。私は (a) を勧める: 仕様 4 は 2 つの行を挙げており、その表の上の規則も明確だからである。修正する人のためのファイル: `ui/src/keys/keyList.ts`、`ui/src/text/ja.ts`、`ui/src/text/en.ts`、`docs/spec/7-readings.md` (G1)、キーの一覧のテスト。

## Fix

選択肢 (a)、リードの決定。`ui/src/keys/keyList.ts` の `OTHER_ROWS`: 「曲とファイル」 「「書き出しました」を閉じる」 (Esc ／ Enter) と、「再生と録音」 「メトロノーム：音量を動かす ／ テンポを手拍子で」 (← → ／ Space Enter)。`ui/src/text/ja.ts`、`en.ts`: `keyList.metronome`、`keyList.closeExported` (`builder:2026-11-25`。仕様 7 の G1 を指すコメント付き)。`docs/spec/7-readings.md` の G1: 「バグ整理で追加した行」という行 (リードが 2026-11-25 に決定。UI-SHELL-008 の、それぞれの言葉の出どころ)。テスト: `ui/tests/keyList.test.ts` の「UI-SHELL-008: メトロノームの窓のキーと「書き出しました」の Enter が一覧にある (仕様 4)」。
