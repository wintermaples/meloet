---
id: CORE-AUDIO-006
title: ボーカルのトラック名が長いと録音ファイルの名前がファイルシステムの上限を超え、声が録音されない
area: CORE-AUDIO
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-AUDIO-006: ボーカルのトラック名が長いと録音ファイルの名前がファイルシステムの上限を超え、声が録音されない

## Where

crates/meloet-core/src/vocalrec.rs:68-77 (`file_name`: トラック名の全体をファイル名に入れる。長さの上限はない)
crates/meloet-core/src/vocalrec.rs:101 (`WavFile::create` は、そのような名前で失敗する)
crates/meloet-model/src/edit.rs:387-405 (`rename_track`: トラック名に長さの上限はない。画面の名前の欄にも `maxlength` はない)

## What happens

録音したファイルは `<track name> <audio id>.wav` という名前になる。トラック名には長さの上限がなく、Linux のファイルシステム (ext4、btrfs、xfs) のファイル名は最大 255 バイト。日本語の一文字は UTF-8 で 3 バイトを使う。ボーカルのトラック名が約 82 文字以上 (ASCII で約 245 文字以上) になると、名前は長すぎる。`WavFile::create` は「File name too long」で失敗し、`start_vocals` は `VocalsNotStarted` を返す。録音は声なしで続く (単語もなし。CORE-AUDIO-003)。同じ関数は「逆にする」(play/reverse.rs:43) と「音のクリップにする」(play/bounce.rs:68-72、接尾辞がつく) のファイルの名前も決める。これらも失敗する。

## What should happen

曲が受け入れるトラック名が、そのトラックの録音を妨げてはならない。人が打った文字からメソッドが作る他のファイル名は、ファイルシステムに安全なものにされている。`vocalrec::file_name` はすでに、`/`、`\`、NUL、制御文字を取り除き、空の名前を扱う (vocalrec.rs:70-75)。`songfile::folder_name` も曲のフォルダについて同じことをする。扱われていないのは長さだけ。

## How to trigger

実行せず、コードを読んで確かめた。

1. ボーカルのトラックの名前を、90 文字の日本語に変える (打ちこむか、AI の link の名前変更で)。
2. そのトラックを録音待ちにして録音する。カウントインと録音は動くが、波形は増えず、声のテイクもできない。

## Cause

`file_name` は、掃除したトラック名、空白、id、`.wav` をつなぐ。バイト長は見ない。`start_vocals` は、使用中の名前に « (n)» を足すので、さらに長くなる。

## Suggested fix

`file_name` で、掃除したトラック名を文字の切れ目で切り、後で足される « (99)» や « 99» (`start_vocals`、`bring_in`、`put_in_place` が足す接尾辞) の分の余裕を残して、名前全体が 255 バイトより十分に短くなるようにする (たとえばトラック名の部分は最大 200 バイト)。プロトコル、ファイルの形式、文言は変えない。既存のファイルの名前には触れない。

## Verification

テストを走らせず、読んで確かめた。`vocalrec::file_name` (`vocalrec.rs:68-77`) は、掃除したトラック名の全体を `<name> <id>.wav` に入れる。`rename_track` はただ前後の空白を除くだけ。約 82 文字の日本語を超えると、ファイル名は 255 バイトを超え、`WavFile::create` は失敗し、`start_vocals` は `VocalsNotStarted` を返す。声は録音されない (黙って。CORE-AUDIO-003)。同じ関数が「逆にする」と「音のクリップにする」のファイルの名前を決める。書き出したファイルについても同じ不具合は CORE-EXPORT-003 にある。重大度 `low` は変えない (とても長いトラック名が必要)。

訂正: なし。

修正案: `crates/meloet-core/src/vocalrec.rs` の `file_name`: 掃除した名前を、id と `.wav` を足す前に、文字の切れ目で最大 200 バイトに切る (そのあと前後の空白と点を再び除く)。これで、後で足される « (n)» と接尾辞の余地が残る。CORE-EXPORT-003 を直すときは、`export/job.rs` の `free_name` と同じ切る処理を共有する (小さな関数を一つ)。テストは `file_name` のテストの隣に置く。100 文字の日本語で、名前は最大 255 バイトになり、` <id>.wav` で終わること。

## Fix

`crates/meloet-core/src/vocalrec.rs` の `file_name`: 掃除したトラック名を、id と `.wav` の前に `crate::export::job::cut_name` (200 バイト、文字の切れ目で、末尾に空白や点を残さない) で切る。同じ関数が「逆にする」と「音のクリップにする」のファイル名も決める。`cut_name` は今の場所に残す (関数は一つ)。`meloet-model` は変えていない。テスト: `vocalrec::tests::a_long_track_name_is_cut_to_fit_a_file_name` (修正がないと失敗する)。
