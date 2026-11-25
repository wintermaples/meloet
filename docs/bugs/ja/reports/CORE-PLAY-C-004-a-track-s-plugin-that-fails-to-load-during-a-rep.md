---
id: CORE-PLAY-C-004
title: 入れかえの試聴中に読みこみに失敗したトラックのプラグインは、Esc の後サイン波ではなく無音のままになる
area: CORE-PLAY-C
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-PLAY-C-004: 入れかえの試聴中に読みこみに失敗したトラックのプラグインは、Esc の後サイン波ではなく無音のままになる

## Where

crates/meloet-core/src/play/plugs.rs:607-624 (`plugin_unreadable`: 席で試聴中のとき、エンジンへの `EngineOp::Instrument { SINE }` がない)
crates/meloet-core/src/play/plugs.rs:509-520 (`place_waiting` は `Ready` のプラグインだけを置く)
crates/meloet-core/src/play/shelf.rs:364-375 (`end_audition` の入れかえモード: `EndTrial { keep: false }` の後に `place_waiting`)
crates/meloet-core/src/play/plugs.rs:446-459 (`seat_instrument`: 読みこみ中のプラグインは `InstrumentRef::Plugin` としてエンジンに送られ、`instruments::make` が `Silent` にする)

## What happens

「入れかえる」モードで、トラックのプラグイン音源がまだ読みこみ中に、そのトラックで別の音を試し始めた後、読みこみが失敗すると、試聴を取り消した後 (Esc、または棚を閉じたとき) にトラックは無音になります。その席のエンジンの音源は、まだ読みこみ中のプラグインの代わりの無音のままだからです。状態行には 「曲で使っている … を読みこめませんでした。1 本のトラックはサイン波で鳴らしています。」 (`PluginUnreadable`) と出て、楽器の行にはサイン波が代わりに鳴ると書かれていますが、弾いても何も鳴りません。

## What should happen

仕様 3 章の帯: 「読みこめなかったプラグイン (89-12)」: 「曲にあるプラグインが読みこめなかったとき、そのトラックはサイン波で鳴り、楽器の行に 「サイン波で代わり」 と出る。」 `plugin_unreadable` の説明文: 「読みこめなかったトラックの音源は、サイン波で鳴る (仕様 3、89-12)」。同じ仲間の `sync_plugs` (plugs.rs:724-731) は、試聴中はサイン波を残しています。そこでは、退けられているものがサイン波だったからです。ここでは、退けられているものが無音の代わりなのです。

## How to trigger

コードを読んだ結果です。実行はしていません。
1. トラック 1 にプラグインの音源がある曲を開く。その音源は読みこみに時間がかかり、その後失敗する (プラグインのホストが 30 s 以内に `Load` に答えない、または読みこみが後で拒否される)。
2. 読みこみ中 (そのトラックは無音) に、トラック 1 の棚を開いて音を入れかえ、行を選ぶ (席での試聴)。
3. 読みこみが失敗する: `plugin_unreadable` が `Phase::Failed` にし、`trial_seat == Some(seat)` のためサイン波は送られない。状態行には、そのトラックはサイン波で鳴ると出る。
4. Esc を押す: `end_audition` が `EndTrial { keep: false }` を送り、その席自身の `Silent` が戻る。`place_waiting` は `Failed` には何もしない。トラック 1 を再生する: 無音。

## Cause

`plugin_unreadable` は、席で試聴中はエンジンがそれを試している音として扱うため、サイン波を送りません。そして、後からサイン波を送る処理がありません。`end_audition` → `place_waiting` が扱うのは `Ready` のプラグインだけだからです。

## Suggested fix

`place_waiting` で (または入れかえモードの `end_audition` で)、その席自身のプラグインが `Phase::Failed` のとき、`EngineOp::Instrument { seat, instrument: InstrumentRef::SINE }` を送る。プロトコル、文言、ファイルの変更はありません。

## Verification

コードを読んで確認しました (テストは実行していません)。`seat_instrument` (`play/plugs.rs:440-459`) は、読みこみ中のプラグインに `InstrumentRef::Plugin` を渡します (代わりの無音)。試聴がその席にあるときに読みこみが失敗すると、`plugin_unreadable` (`plugs.rs:607-624`) は `Phase::Failed` にし、サイン波を残しません (`trial_seat != Some(seat)` は偽)。しかし、状態行 `StatusMessage::PluginUnreadable` は返します。Esc を押すと、`end_audition` (`play/shelf.rs:364-375`) が `EndTrial { keep: false }` を送り、`place_waiting` を呼びます。それは `Ready` のプラグインだけを扱います。そのため、席自身の音源は無音の代わりのままです。これは仕様 3 章 (89-12) と、今表示された状態行に反します。重大度 `low` はそのままです (その同じトラックの入れかえ試聴中に、読みこみが後で失敗する場合だけ)。

Corrections: 訂正なし。

修正の計画: `crates/meloet-core/src/play/plugs.rs` の `place_waiting`: 席自身のプラグインが `Phase::Failed` のとき、`EngineOp::Instrument { seat, instrument: InstrumentRef::SINE }` を送る。入れかえ試聴のもう一方の端も確認する (試した音を残す場合は、自分のプラグインを捨てるので何も要らない)。テスト (棚かプラグインのテスト): 試聴中に読みこみが失敗し、Esc を押すと、席はサイン波を受け取る。

## Fix

`crates/meloet-core/src/play/plugs.rs` の `place_waiting`: 席自身のプラグインが読みこめなかった (`Phase::Failed`) とき、`EndTrial` の後に、エンジンはその席に `EngineOp::Instrument { SINE }` を受け取る。試した音が残るときは、自分のプラグインを捨てるので、それ以上は何も要りません (`play/shelf.rs` は変更なし)。

テスト: `play::plugs::tests::a_tracks_plugin_that_does_not_load_under_a_trial_sounds_as_the_sine_wave_after_it`。
