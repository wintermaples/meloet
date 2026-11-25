---
id: CORE-PLAY-C-003
title: 入れかえの試聴の下で準備ができたトラックのプラグインは、サンプルレートが変わるとその状態を失う
area: CORE-PLAY-C
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-PLAY-C-003: 入れかえの試聴の下で準備ができたトラックのプラグインは、サンプルレートが変わるとその状態を失う

## Where

crates/meloet-core/src/play/plugs.rs:661-665 (`live` と `Phase::Ready` → `PlugState::Of(old.ticket)`)
crates/meloet-core/src/play/plugs.rs:674-676 (`Ready && !placed` → `Effect::DropPlugin { ticket: old.ticket }`。`LoadPlugin` の前に積まれる)
crates/meloet-core/src/play/plugs.rs:681-688 (`PlugState::Of(old.ticket)` を使う `LoadPlugin`)
crates/meloet-core/src/service.rs:3338-3348 (`DropPlugin` は `ready[old]` と `ids[old]` を外し、そのプラグインをアンロードする)
crates/meloet-core/src/service.rs:3649-3652 (`ids[other]` がない `PlugState::Of(other)` → `StartState::Fresh`)

## What happens

出力が別のサンプルレートで開くと（`set_plug_rate` → `load_again(true)`）、トラックの専用プラグインは読み込まれていてもエンジンには入っていない状態になります。そのとき、プラグインは起動の途中で、状態を持たないまま読み込み直されます（`StartState::Fresh`）。曲にはそのパッチや設定が残っていますが、新しいインスタンスには入りません。プラグインは既定の音で鳴り、保存すると、`plugin_tickets` がプラグインに状態を聞き、曲の状態ではなく既定の状態をファイルに書きます。

トラックのプラグインは、棚がそのトラックで「入れかえる」モードの別の音を試聴している間だけ、「準備ができていて、置かれていない」状態になります（`plugin_loaded`: `Spot::Seat(seat) if trial_seat == Some(seat) => plug.placed = false`）。

## What should happen

別のレートのために読み込み直されたプラグインは、その音を保ちます。`load_again` は「`live`: 今持っている状態で」と言い、テスト `plugins_are_loaded_again_for_another_sample_rate_and_for_a_new_engine`（state/added_tests.rs:1388）は「今の音で」という `PlugState::Of(ticket)` を期待します。曲の中のプラグインの状態は、決して失われてはなりません（CLAUDE.md、曲のファイルの決まり。アーキテクチャ 20-6）。

## How to trigger

コードを読んで確認したもので、実際には動かしていません。
1. トラック 1 に、保存したパッチのあるプラグインの音源がある曲を開きます（または、出力をまだ選んでいない状態で meloet を起動し、プラグインを 48 kHz で読み込ませます）。
2. そのプラグインがまだ読み込み中に、トラック 1 で棚を「入れかえ」モードで開き、別の行を試します（その座席での試聴）。プラグインの読み込みが終わります: `Ready`、`placed = false`。
3. 44.1 kHz で動く出力を選びます: `AudioOpened` → `set_plug_rate(44_100)` → `load_again(true)`。エフェクトは `DropPlugin { old }` の後に `LoadPlugin { new, state: Of(old) }` になります。サービスは順に処理し、二つ目は `ids[old]` を見つけられず、`Fresh` で読み込みます。
4. Esc を押すと、座席自身の音源が戻ります（`place_waiting`）。音は既定のままです。保存すると、ファイルには既定の状態が入ります。
同じ試聴の間にサンプルレートが二回変わる場合も、同じ経路になります（最初の変更での読み込み直しが試聴の下に入り、二回目では `Ready && !placed` になります）。

## Cause

`load_again` では、置かれていない古いインスタンスの `DropPlugin` を、そのインスタンスの状態をチケットで求める `LoadPlugin` より前に積んでいます。`Effect::DropPlugin` は、チケットを `PluginSide::ids` から外すので、`load_plugin` はその状態を読むインスタンスを見つけられず、`StartState::Fresh` に戻ります。

## Suggested fix

`DropPlugin { ticket: old.ticket }` を `LoadPlugin` の後に積みます（ホストのスレッドは順に答えるので、まず読み込みのためにもとのインスタンスの状態を読み、それからアンロードします）。または、`!old.placed` の場合は `PlugState::Bytes(old.plugin.state)` を使います（空なら `Fresh`）。置かれていないインスタンスは、一度も変更されていないからです。プロトコルやファイルの変更はありません。

## Verification

コードを読んで確認しました（テストは実行していません）。`load_again`（`play/plugs.rs:659-690`）では、`Ready` のプラグインは、`live` の場合 `PlugState::Of(old.ticket)` になります。さらに `!old.placed` の場合、`Effect::DropPlugin { ticket: old.ticket }` が `Effect::LoadPlugin` の前に積まれます。サービスはエフェクトを順に処理します。`DropPlugin`（`service.rs:3338-3348`）が `ready[old]` と `ids[old]` を外し、次に `load_plugin`（`service.rs:3649-3652`）は `PlugState::Of(old)` の `ids[old]` を見つけられず、`StartState::Fresh` で読み込みます。座席自身のプラグインが `Ready` で置かれていないのは、その座席で入れかえの試聴が動いている間だけです（`plugin_loaded`、`plugs.rs:549`）。そのためプラグインは既定の状態になり、曲はまだ保存された状態を持っていて、保存時にはプラグインに状態を聞きます。重大度 `medium` のままです。まれな順序の出来事です（入れかえの試聴の間に、その座席の専用プラグインが試聴の下で準備でき、そこで出力が別のレートになる）が、保存された曲はプラグインの状態を失います。

訂正はありません。

修正の計画: `crates/meloet-core/src/play/plugs.rs` の `load_again`: `Ready && !placed` では、インスタンスの状態ではなく曲の状態を使います（`PlugState::Bytes(old.plugin.state.clone())`、空なら `Fresh`）。エンジンに入っていないインスタンスは、一度も演奏も変更もされていないので、両者は同じです。二つのエフェクトの順序も関係なくなります。`DropPlugin` は残します。テスト（`state/added_tests.rs`）: 試聴の間にサンプルレートが変わると、`Bytes(the song's state)` を持つ `LoadPlugin` になる。

## Fix

`crates/meloet-core/src/play/plugs.rs` の `load_again_at`（`load_again` の本体）: エンジンに入っているプラグインだけが、そのチケットの `PlugState::Of` で読み込み直されます。試聴の下で準備ができ、一度も置かれていないプラグインは、曲の状態（`Bytes`、空なら `Fresh`）を受け取ります。`DropPlugin` は残ります。

テスト: `play::plugs::tests::a_tracks_plugin_that_became_ready_under_a_trial_keeps_the_songs_state_at_another_rate`。
