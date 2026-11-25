---
id: CORE-EXPORT-002
title: 書き出しの保存の窓に入力したファイル名は、最後のドットより後ろの文字が失われる
area: CORE-EXPORT
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-EXPORT-002: 書き出しの保存の窓に入力したファイル名は、最後のドットより後ろの文字が失われる

## Where

crates/meloet-core/src/export.rs:738-741 (`ExportNews::PlacePicked`: `path.file_stem()` を `self.name` にする)
crates/meloet-core/src/export.rs:896-899 (`file_name`: `format!("{name}.{}", self.format.extension())`)

## What happens

「えらぶ…」の保存の窓で選んだ名前は `Path::file_stem` として保たれる。これは、そのあとの部分が音声の拡張子かどうかに関係なく、最後のドットより後ろをすべて切り落とす。そのあと、形式の拡張子が付けられる。入力欄に「デモ v1.2」と入れた人 (拡張子なし) は、「デモ v1.wav」というファイルを得る。「Mix 2026.11.25」は「Mix 2026.11.wav」になる。
さらに悪いことに、保存の窓が上書きの確認をしたのは、入力された名前についてである (`set_do_overwrite_confirmation(true)`、crates/meloet-app/src/pcwindow.rs:132-135)。実際に書かれる名前についてではない。フォルダに「デモ v1.wav」がすでにあると (以前の書き出し)、何も聞かれずに上書きされる。

## What should happen

置き換えるのは、形式の拡張子だけであるべきである (「The ending is the format's, whatever was typed」。export/tests.rs、`ChooseExportPlace` のテスト)。音声の拡張子でない文字は、名前の一部として残す。兄弟の関数 `split_save_as` (crates/meloet-core/src/state/songs.rs:565-571) は、「名前をつけて保存…」でこれを正しく行っている。`.meloet` の接尾辞だけを取り、残りの名前は残す。

## How to trigger

実行して確かめたのではなく、コードを読んで確かめた。
1. 一度「~/音楽/デモ v1.wav」へ書き出す。
2. 「書き出し…」をもう一度開き、「えらぶ…」を押す。保存の窓に「デモ v1.2」と入力して保存を押す (そのファイルはないので、確認は出ない)。
3. ダイアログの場所には「~/音楽/デモ v1.wav」と表示される。「書き出す」を押すと、最初のファイルが上書きされる。

## Cause

`file_stem()` は、最後のドットを、どんな拡張子の始まりとしても扱う。ドットのある名前 (版や日付) は普通に使われる。

## Suggested fix

`PlacePicked` で、ファイル名全体を取り、meloet が書く拡張子 (`wav`、`flac`、`mp3`、`mid` で、大文字小文字は問わない) だけを取る。`split_save_as` が `.meloet` について行うのと同じである。それ以外はファイル名全体を残す。プロトコル、文言、見た目は変えない。テスト: 「デモ v1.2」は場所「…/デモ v1.2.wav」になる。「できた.FLAC」は、wav のもとでは「…/できた.wav」になる。

## Verification

コードを読んで確かめた (テストは実行していない)。`ExportNews::PlacePicked` (`export.rs:738-741`) は `path.file_stem()` を名前として保つ。`file_name` (`export.rs:896-899`) は形式の拡張子を付ける。`Path::file_stem` は、その後ろが何であれ最後のドットで切るので、「デモ v1.2」は「デモ v1.wav」になる。GTK の保存の窓は、入力された名前について上書きを確認する (`pcwindow.rs:132-135`)。書き出し自体は、完成したファイルを最終的な名前のものの上へ移す (`export/job.rs` の `move_to`。ただの名前の変更)。そのため、切られた名前は、以前の書き出しを、窓が聞くべき質問なしに置き換えてしまう。`split_save_as` (`state/songs.rs:565-571`) は、自分の拡張子だけを取る。

重大度は、発見者も考えたとおり `low` から `medium` に変えた。ドットのある名前 (版、日付) は普通の使い方で、ファイルは使う人が入力していない名前になり、切られた名前の以前の書き出しは、窓が確認するはずの質問なしに置き換えられる。`high` ではない。失われるのは書き出したファイルであり、曲ではないからである。

訂正: なし。

修正計画: `crates/meloet-core/src/export.rs` の `PlacePicked`: `path.file_name()` を取り、meloet が書く拡張子 (`ExportFormat` の拡張子。大文字小文字は問わない: wav、flac、mp3、mid) だけを取る。ほかは残す。テストは `export/tests.rs` の `ChooseExportPlace` のテストの隣に置く: 「デモ v1.2」→「デモ v1.2.wav」、wav のもとで「できた.FLAC」→「できた.wav」。

## Fix

`crates/meloet-core/src/export.rs` の `ExportNews::PlacePicked`: 名前は、入力されたファイル名全体から、選ばれた形式自身の拡張子だけを、大文字小文字を問わず取り除いたものになる (`typed_name`)。ほかのドットの後ろの文字は残る (「デモ v1.2」→「デモ v1.2.wav」。「できた.flac」を wav のもとで → 「できた.flac.wav」。リードの決定)。書かれるファイルが、窓が確認を求めた名前と違い (拡張子なしで入力されたか、大文字が違う)、そのファイルがすでにあるとき、場所は取られない。ダイアログは `again` を保ち、`Exporting::settle` が、そのファイルの名前で保存の窓をもう一度出す。そのため、PC 自身の「上書きしますか?」という質問は、そのファイルについてのものになる。閉じると、場所は前のままになる。新しい文言は作らない。`docs/architecture.md` 22-20 (書き出しの場所) はこのことを書いている。

テスト: `export::tests::a_name_typed_keeps_its_dots_and_only_the_formats_ending_is_taken_off` と `export::tests::the_window_comes_again_to_ask_about_the_file_really_written` (`export/tests.rs`)。
