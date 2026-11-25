---
id: CORE-EXPORT-003
title: トラック名がファイル名の長さの上限を超えると、トラックごとの書き出しが全体として失敗する
area: CORE-EXPORT
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-EXPORT-003: トラック名がファイル名の長さの上限を超えると、トラックごとの書き出しが全体として失敗する

## Where

crates/meloet-core/src/export/job.rs:299-313 (`free_name`)
crates/meloet-core/src/export/job.rs:259-264, 286 (フォルダに名前を付けたあと、`move_to`)
crates/meloet-core/src/export/midi.rs:342-348 (MIDI の「トラックごと」も同じ `free_name` を使う)

## What happens

「トラックごと」は、各ファイルにトラックの名前を付けるために `free_name` を使います。それは `/`、`\`、NUL、制御文字を取り除きますが、長さは制限しません。トラック名には長さの上限がありません（`meloet_model::edit::rename_track` は前後の空白を取るだけです）。Linux のファイルシステムでは、ファイル名は最大 255 バイトです。日本語の文字は UTF-8 で 3 バイトなので、約 84 文字以上のトラック名では、`move_to`（音声）または `write_beside`（MIDI）が ENAMETOOLONG で失敗します。すると書き出し全体が「書き出せませんでした」（`ExportFailure::CannotWrite`。「ディスクがいっぱいか、場所が受け付けない」）になり、すでに移したファイルは消されます。そして、1 つのトラックの名前が原因だとは、どこにも知らされません。

## What should happen

`free_name` は、「ファイル名に入れられないものを取り除いて」`<name>.<ending>` を作ると言っています。ファイルには長すぎる名前も、そのようなものの 1 つです。すべてのトラックにファイルが 1 つずつ作られるべきです（spec 3「トラックごとは、トラックごとに 1 つのファイルを、トラック名で作る」）。

## How to trigger

コードを読んで確かめました。実行はしていません。
1. トラックの名前を、日本語 90 文字の名前に変えます。
2. 「書き出し…」、「トラックごと」、任意のフォルダ、「書き出す」。
3. 実行は「書き出せませんでした」で失敗します。ファイルは 1 つも残りません。

## Cause

`free_name` にはバイト数の上限がなく、形式の拡張子と " 2" の番号は、名前のあとに付けられます。

## Suggested fix

`free_name` で、`clean` を文字の区切りで切り、`clean`、番号の部分、`.<ending>` を合わせて 255 バイト以内に収めます（たとえば、番号と拡張子を付ける前に、`clean` を 200 バイトに切る）。音声と MIDI の「トラックごと」の両方が、それを使います。プロトコル、文言、見た目は変えません。

## Verification

コードを読んで確かめました（テストは実行していません）。`free_name`（`export/job.rs:299-313`）は、`/`、`\`、NUL、制御文字を取り除き、長さの上限はありません。`rename_track` は名前の前後の空白を取るだけです。約 84 文字を超える日本語のトラック名では、ファイル名が 255 バイトを超え、`move_to`（または MIDI の書き出し、`export/midi.rs:342-348`）が失敗し、トラックごとの書き出し全体が `ExportFailure::CannotWrite` になります。重要度 `low` は変えません（非常に長いトラック名の場合だけのため）。

訂正: なし。

修正案: `crates/meloet-core/src/export/job.rs` の `free_name`: `clean` を文字の区切りで切り、名前、番号、`.<ending>` を合わせて 255 バイト以内に収めます（`clean` を最大 200 バイトに切ったあと、もう一度空白とドットを取る）。`free_name` のテストの横に、テストを 1 つ置きます。100 文字の日本語は、拡張子で終わる 255 バイト以下の名前になり、そのような 2 つのトラックは別々の名前になることを確かめます。

## Fix

`crates/meloet-core/src/export/job.rs`: 共通の補助関数 `cut_name(name: &str) -> &str`（`NAME_BYTES = 200`。文字の区切りで切り、末尾に空白もドットも残さない）。`free_name` は、整えたトラック名を、" 2" の番号と拡張子を付ける前に切ります。そのため名前は 255 バイト以内に収まり、名前は今まで通り互いに違います。音声と MIDI の「トラックごと」（さらに AI 連携のファイル一覧、`state/ai.rs`）は、すべて `free_name` を使います。

テスト: `export::job::tests::a_track_name_too_long_for_a_file_name_is_cut_between_characters`（`export/job/tests.rs`）。
