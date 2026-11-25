---
id: AI-002
title: write_notes、copy_notes、import_midi の文面は、短くなったノートについて "see changed" と言う。フィールドは `cut` である
area: AI
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# AI-002: write_notes、copy_notes、import_midi の文面は、短くなったノートについて "see changed" と言う。フィールドは `cut` である

## Where

crates/meloet-ai/src/tools/notes.rs:260-262 (`notes_line`)

## What happens

いくつかのノートが短く書かれたとき、`write_notes`、`copy_notes`、`import_midi` の答えの最初の行は "N written shorter (see changed)." と言う。答えには `changed` という一覧はない。短くなったノートは `cut` にあり (`cut_count`、`cut_more` も付く)、`changed` は、書き込みが成功したことを示す真偽値 `true` である。文面に従う AI は、違うフィールドを見てしまう。

## What should happen

文面は、答えが持つフィールド名 `cut` を指すこと (docs/development/ai-tools.md の「3 と 10」: 「`cut` | 切られたノートを最大 200 個 …」。同じファイルの `notes_value` は `value["cut"]` を書く)。

## How to trigger

コードを読んで分かったこと。実行はしていない。新しいクリップで小節 9999 を越えて `write_notes` する。または、クリップの終わりがノートを切る場合 (たとえば `MAX_TICK` の近くに長いノート)。文面の行に "(see changed)" と出る。

## Cause

JSON のフィールドは `cut` と名付けられている (`notes_value`、229 行目の `value["cut"] = json!(changed)`。ローカル変数は今も `changed` と呼ばれる) が、`notes_line` の文面は古い名前のまま残っている。

## Suggested fix

`notes_line` の "(see changed)" を "(see cut)" に変える。AI 向けの文面 (英語。画面の文言ではない)。プロトコルは変えない。

## Verification

コードを読んで確認した。`notes_line` (`crates/meloet-ai/src/tools/notes.rs:260-262`) は "N written shorter (see changed)." と書く。同じファイルの `notes_value` は、短くなったノートを `cut` に入れ (`cut_count`、`cut_more` も)、答えの `changed` は書き込みが成功したことの真偽値である。ツールの説明 (`tools.rs:106`) も「not_written と cut を読む」と言っている。重大度 `low`。

修正案: `crates/meloet-ai/src/tools/notes.rs`: "(see cut)"。古い文面を固定しているテストがあれば (`grep "see changed" crates/meloet-ai`)、それも一緒に変える。

## Fix

`crates/meloet-ai/src/tools/notes.rs` (`notes_line`): "(see cut)"。古い文面を固定したテストはなかったので、テストは足さなかった (1 行の文面)。
