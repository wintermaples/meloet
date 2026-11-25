---
id: AI-003
title: list_effects の説明は、meloet の内蔵エフェクトが六つだと書いているが、実際は九つ
area: AI
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# AI-003: list_effects の説明は、meloet の内蔵エフェクトが六つだと書いているが、実際は九つ

## Where

crates/meloet-ai/src/tools/params.rs:582-585（`list_effects` の doc コメント。rmcp が `tools/list` でツールの説明として送る）

## What happens

AI のアプリが読む説明には「meloet 自身の六つを、そのパラメーターとともに（最初のページに）」とあります。答えの `builtin` には九つが入ります: eq4、compressor、reverb、delay、chorus、limiter、gate、deEsser、saturation（`EffectKind::ALL` は `[EffectKind; 9]`、crates/meloet-model/src/effects.rs:417）。`insert_effect` 自身の引数の説明（mixer.rs:222-223）は九つすべてを挙げています。

## What should happen

説明は、ツールが実際に返すものと一致するべきです: meloet の内蔵エフェクトは九つ（または数を書かない）。タスクの説明と docs/development/ai-tools.md は、各ツールの説明が実際の動きと一致することを求めています。隣の `EffectArgs.effect` の説明は九つを挙げています。

## How to trigger

コードを読んで確認したもので、実際には動かしていません。MCP のクライアントに `tools/list` を頼み、`list_effects` の説明を読みます。`list_effects` を呼び、`builtin` を数えます。

## Cause

gate、deEsser、saturation はラウンド 12 で足されました（CLAUDE.md: エフェクトの種類 `gate`、`deEsser`、`saturation`）。説明は更新されていませんでした。

## Suggested fix

doc コメントに「meloet 自身の九つ」（または「meloet 自身のエフェクト」）と書きます。説明は 60 語以内に保ちます（`every_tool_says_what_it_is`）。プロトコルの変更はありません。

## Verification

コードを読んで確認しました。`list_effects` の doc コメント（`crates/meloet-ai/src/tools/params.rs:582-585`）は、ツールの説明ですが、「meloet 自身の六つ」と書いています。`EffectKind::ALL` は九つあり（gate、deEsser、saturation はラウンド 12 から）、答えの `builtin` はそのすべてを挙げています。重大度 `low` のままです。

修正の計画: `crates/meloet-ai/src/tools/params.rs`: 「meloet 自身の九つ」。語数は同じなので、`every_tool_says_what_it_is` は通ります。`docs/development/ai-tools.md` に同じ数字があり六つと書いてあれば、そこも変えます。

## Fix

`crates/meloet-ai/src/tools/params.rs`: `list_effects` の説明は「meloet 自身の九つ」になりました（説明は、rmcp のマクロが読む doc コメントなので、`EffectKind::ALL` から数えることはできません）。`docs/development/ai-tools.md` には数字はありませんでした。見張りは `every_tool_says_what_it_is`（語数）だけです。
