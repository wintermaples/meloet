---
id: UI-ROLL-001
title: ピアノロールのツールバーのテイクのボタンが、表示中のクリップではなくアレンジで選ばれたクリップのテイクを切りかえる
area: UI-ROLL
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-ROLL-001: ピアノロールのツールバーのテイクのボタンが、表示中のクリップではなくアレンジで選ばれたクリップのテイクを切りかえる

## Where

- `ui/src/components/pianoroll/Toolbar.svelte:77-81` — `takeTag` は `shown.clip` (ピアノロールに表示しているクリップ、`arrange.pianoRollClip`) から作られる。
- `ui/src/components/pianoroll/Toolbar.svelte:225` と `:234` — ◂ ▸ は `{ type: "stepTake", delta }` を送る。クリップの指定はない。`takeTag.recording` のときだけ使えなくなる。
- `crates/meloet-core/src/play.rs:1295-1299` — `Play::step_take` は `self.selected` (`arrange.selectedClip`) に対して動く。
- `crates/meloet-core/src/play.rs:1055-1060` — `select_clip(None)` (アレンジの何もない所を押す) は `selected` を消すが、`piano_roll` は残す。
- `crates/meloet-protocol/src/lib.rs:2810-2816` — `selected_clip` と `piano_roll_clip` は、わざと分けてある (「選択をはずしたあとも残す」)。

## What happens

ピアノロールのツールバーにあるテイクの札「◂ テイク 2 / 3 ▸ T」は、ピアノロールに表示しているクリップのテイクを示しますが、◂ ▸ はアレンジで選ばれているクリップのテイクを切りかえます。二つは同じクリップではないことが多いです。
- アレンジの何もない所を押すと、クリップは選ばれなくなりますが、ピアノロールはクリップと札を残したままです。このとき ◂ ▸ は何もしません。
- アレンジで囲んで選ぶと、トラック2のクリップBだけが選ばれて残ります (コアは、表示中のクリップではなく、クリックされたクリップを選ぶ。`crates/meloet-core/src/play/arrange_tests.rs:289-295` のテストを参照)。このとき ◂ ▸ はそのほかのクリップのテイクを切りかえます。その音が変わり、取り消しの手も作られますが、ピアノロールと札は変わりません。
- カウントイン中、または別のトラックで録音中でも、◂ ▸ は使えそうに見えますが、コアは何もしません (`step_take` は `self.busy()` で戻る)。その間、アレンジの「次のテイク」「前のテイク」の行は使えなくなっています。

## What should happen

仕様 3「第 3 版の決まり」: 「テイクが 2 つ以上あるクリップをピアノロールで開くと、同じ切りかえの札 (「◂ テイク 2 / 3 ▸ T」) が、ツールバーのトラック名の横に出る」。札はピアノロールが表示しているクリップのものなので、◂ ▸ はそのクリップのテイクを切りかえるべきです (アレンジの札は、描かれているクリップに対して動く。`Arrange.svelte` の `switcher` は `selected` から)。読み値 A-8: 録音中は「次のテイク」「前のテイク」を使えなくする。

## How to trigger

コードを読んで確かめたもので、実行はしていません。
1. トラック1のクリップAに、テイクを2つ録音する。アレンジでAを押す。ピアノロールはAを表示し、「◂ テイク 2 / 2 ▸ T」を出す。
2. アレンジのレーンの何もない所を押す (ドラッグはしない)。`selectClip { clip: null }` が送られる (`Arrange.svelte:1424`)。ピアノロールは札と一緒にAを表示したままになる。
3. ピアノロールのツールバーで ▸ を押す。何も変わらない。
変わり方: 手順2のあとで、トラック2のクリップB (テイクが2つ以上) だけを囲む箱を描いて選ぶ。ピアノロールで ▸ を押すと、Bのテイクが変わり、Aは変わらない。

## Cause

ツールバーは `shown.clip` から何を表示するかを決めますが、送るのは `StepTake` で、クリップのIDを含みません。コアは `self.selected` を読みます。これは、アレンジの何もない所を押したあと、囲んで選んだあと、Mod A のあと、1つだけ選んだ箱のあとでは、ピアノロールのクリップではありません。ボタンの使えない状態は `takeTag.recording` (このクリップに録音中か) だけを見ていて、カウントインや別の場所での録音は見ていません。

## Suggested fix

◂ ▸ が表示中のクリップに対して動くようにします。方法は二つです。(a) `StepTake` にクリップのIDを任意で持たせる (または `DeleteTake { clip }` のように、クリップを付けた意図を新しく作る)。ツールバーは `shown.clip.id` を送る。これはプロトコルの変更です。(b) プロトコルを変えずに、`arrange.selectedClip === shown.clip.id` でないとき、そしてカウントイン中や録音中も、◂ ▸ を使えなくする (札の「T」の案内も外す)。(b) では上の場合に札が使えないままになります。どちらにするかは、リードが決めることです。

## Verification

コードを読んで確かめたもので、実行はしていません。札は表示中のクリップから作られ (`Toolbar.svelte:77-81`)、そのボタンは `stepTake` をクリップの指定なしで送ります (225、234)。`step_take` は `self.selected` に対して動きます (`crates/meloet-core/src/play.rs:1299`)。`select_clip(None)` は `selected` を消し、ピアノロールのクリップは残します (1055-1060)。そのため、アレンジの何もない所を押したあとはボタンが何もせず、ほかのクリップが選ばれたままの箱のあとでは、そのほかのクリップが変わります。また、カウントイン中や別の場所で録音中でもボタンは使えないままです (`disabled={takeTag.recording}` だけ)。そこでは `step_take` が `busy()` で戻ります。この半分の原因は UI-ROLL-008 / UI-ARRANGE-009 と同じです。

プロトコルを変えずに直せるので、`needs-lead` ではなく `confirmed` にします。

修正計画: `ui/src/components/pianoroll/Toolbar.svelte` で、`current.state.arrange.selectedClip !== clip.id` のとき (アレンジが別のクリップを選んでいる、または何も選んでいない)、`stepTake` の前に `{ type: "selectClip", clip: clip.id }` を送る (どちらも `send` を通すので順番は保たれる)。`selectClip` は取り消しの手ではなく、ピアノロールはすでにそのクリップを表示しているので、動くのはアレンジの選択だけです。これは、テイクを切りかえたクリップをアレンジの札が示すために必要でもあります。カウントイン中 (`transport.state` が `recording` か `countIn`) も ◂ ▸ を使えなくする。テスト: 表示中のクリップと選ばれているクリップが違うとき、二つの意図がこの順で送られる。

もう一つの方法は、リードがアレンジの選択を動かしたくない場合に使います: `StepTake` にクリップのIDを持たせる (プロトコルのバージョンを上げる。T キーとメニューの行は選ばれているクリップを送る)。修正には必要ありません。

## Fix

プロトコルの変更はなし。

- `ui/src/components/pianoroll/Toolbar.svelte`: アレンジが別のクリップを選んでいる (または何も選んでいない) とき、◂ ▸ は先に表示中のクリップの `selectClip` を送り、それから `stepTake` を送る。録音中やカウントイン中 (`songBusy`) は使えなくする。メニューの「次のテイク」「前のテイク」と同じ決まり。
- テスト: `ui/tests/pianoroll.band.test.ts` の「◂ ▸ switch the take of the shown clip: when the arrange chose another clip or none, the shown one is chosen first (UI-ROLL-001)」と「◂ ▸ cannot be used while counting in or recording, as the menu's 「次のテイク」「前のテイク」 (UI-ROLL-001)」。
