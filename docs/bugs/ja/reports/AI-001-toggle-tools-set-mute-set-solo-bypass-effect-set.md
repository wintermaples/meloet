---
id: AI-001
title: 切りかえのツール (set_mute、set_solo、bypass_effect、set_loop_on) は、まず状態を読み、それから切りかえの意図を送るため、二つの呼び出しが同時だと元に戻ってしまう
area: AI
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# AI-001: 切りかえのツール (set_mute、set_solo、bypass_effect、set_loop_on) は、まず状態を読み、それから切りかえの意図を送るため、二つの呼び出しが同時だと元に戻ってしまう

## Where

crates/meloet-ai/src/tools/mixer.rs:450-472 (`set_mute`: `muted` を読み、それから `Command::ToggleMute` / `ToggleReturnMute` を送る)
crates/meloet-ai/src/tools/mixer.rs:489-494 (`set_solo`: `soloed` を読み、それから `ToggleSolo` を送る)
crates/meloet-ai/src/tools/mixer.rs:597-609 (`bypass_effect`: `bypassed` を読み、それから `ToggleBypass` を送る)
crates/meloet-ai/src/tools/timing.rs:293-300 (`set_loop_on`: `loop_on` を読み、それから `ToggleLoop` を送る)

## What happens

この四つのツールは、望む状態 (`on: true/false`) を受け取りますが、曲を変えるには切りかえの意図を使います。ツールは曲を読み、比べ、違えば切りかえを送ります。読んでから、コアが切りかえを実行するまでの間に、同じスイッチを切りかえるほかの操作があると、切りかえは逆向きに働きます。同時に走る二つの `set_mute {track: 2, on: true}` はどちらも「ミュートされていない」と読み、どちらも `ToggleMute` を送ります。トラックはミュートされていない状態で終わり、どちらの答えも `changed: true` (「完了」) になります。使う人が M (または S、バイパスのボタン、L) を、その読んでから送るまでの間に押した場合も同じです。これらのツールには `idempotent_hint = true` が付いており、AIのアプリは同じ呼び出しを送り直したり、いくつかの呼び出しを同時に送ったりすることがあります。

## What should happen

`set_mute {on: true}` は、同時に何が走っていても、トラックをミュートした状態のままにします。二つ目の呼び出しは「変更なし」です (docs/development/ai-tools.md、「変更なし」: 「値を切りかえに変えるツール (`set_mute`、`set_solo`、`bypass_effect`、`set_loop_on` …) は、すでにその状態なら同じ答えを返す」)。ツールの `idempotent_hint` もそう言っています。

## How to trigger

コードを読んで確かめたもので、実行はしていません。一つの接続で、最初の答えを待たずに `tools/call` の要求を二つ送る: `set_mute {"track": 1, "on": true}` (rmcp 3.11 の `service.rs` は、接続の要求をそれぞれ独立したタスクとして `spawn_service_task` で走らせるため、どちらの切りかえもコアに届く前に両方が曲を読みます)。または、同時に接続している二つのAIアプリ (最大8接続)。または、その呼び出しが読んでから頼むまでの間に、使う人がそのトラックの M を押す。

## Cause

状態はAIのスレッドで読まれ (`self.read()`)、変更はコアのスレッドが後で実行する切りかえです。読んだときの状態のままかどうかを確かめる仕組みがありません。AI の呼び出しの場合、読んでからコアへ頼むまでの間に、使う人の意図やほかの接続の要求を止める仕組みはありません (一括の場合を除く)。

## Suggested fix

コアに望む値で決めさせます。AIの要求として、例えば `AiAsk::SetToggle { what, on }` (または、保護を付けた既存の画面のコマンドの経路) を作り、コアのスレッドで、スイッチがすでに `on` なら何もしない (`Unchanged`)、そうでなければ切りかえを送る。`AiAsk` はコアの内部のものなので、画面へのプロトコルは変わりません。新しい文面もありません。

## Verification

コードを読んで確かめたもので、実行はしていません。`set_mute`、`set_solo`、`bypass_effect`、`set_loop_on` はそれぞれ、AI側で曲を読み (`self.read()`)、`on` と比べ、それから `AiAsk::Screen(Command::Toggle…)` で単純な切りかえを送ります。この切りかえのコアの空実行 (`crates/meloet-core/src/play/ai.rs:279-341`) は望む値を知らないため、切りかえが実行されるときにスイッチが読んだときのままかを確かめるものがありません。同じツールの呼び出しが二つ同時に来る場合や、読んでから頼むまでの間に使う人が M / S / L を押す場合に、切りかえは逆向きになり、どちらの答えも `changed: true` になります。ツールには `idempotent_hint = true` が付いています。重大度 `low` は変えません。数ミリ秒の間に二つの要求が入る必要があるためです。

訂正: なし。

修正計画: コアのスレッドに決めさせます。`crates/meloet-core/src/ai.rs`: 切りかえとともに望む値を運ぶ要求 (例: `AiAsk::ScreenWhen { command, on: bool }`。コアの内部で、画面の契約には含まれない)。`crates/meloet-core/src/play/ai.rs` / `state/ai.rs`: コアのスレッドで、そのコマンドが切りかえるスイッチ (トラックやリターンのミュート、ソロ、インサートのバイパス、`loop_on`) を読み、`on` ならすでに `AiRefusal::Unchanged` を返し、そうでなければ今のとおり切りかえを実行する。`crates/meloet-ai/src/tools/mixer.rs` と `tools/timing.rs` はこれを送り、よくある場合のための早い「変更なし」は残す。テスト: `crates/meloet-ai/tests/link.rs` で、`on: true` の要求を二回続けると、トラックはミュートされたままになり、二つ目は「変更なし」になる。

## Fix

`crates/meloet-core/src/ai.rs`: コア内部の要求 `AiAsk::ScreenWhen { command, on }` (状態 `on` への切りかえ)。`crates/meloet-core/src/state/ai.rs` (`ai_write`): コアのスレッドで、スイッチがすでに `on` なら `AiRefusal::Unchanged` を返し、そうでなければ `AiAsk::Screen` と同じく切りかえを実行する。`crates/meloet-core/src/play/ai.rs`: `Play::ai_switch` は、切りかえが反転させるスイッチを読む (トラックのミュート、リターンのミュート、ソロ、バイパス、`loop_on`)。`crates/meloet-ai/src/tools.rs` (`screen_when`)、`tools/mixer.rs` (`set_mute`、`set_solo`、`bypass_effect`)、`tools/timing.rs` (`set_loop_on`) はこれを送り、早い「変更なし」は残す。プロトコルの変更はなし。文書: `docs/development/ai-tools.md`、`docs/architecture.md` (24-16)。

テスト: `crates/meloet-ai/tests/link.rs` の `a_toggle_for_a_state_ends_in_that_state` (四つの切りかえのそれぞれを、`on` で二回要求する。どちらも、どちらかが meloet に届く前に読んだ二つの呼び出しとして扱う。一つ目が切りかえ、二つ目は `Unchanged`。状態は要求どおりで終わる。`on` が true と false の両方)。
