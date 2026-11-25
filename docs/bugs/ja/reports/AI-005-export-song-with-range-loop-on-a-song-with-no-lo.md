---
id: AI-005
title: ループ範囲がない曲で export_song の range に "loop" を指定すると、曲全体を書き出し、そのことを知らせない
area: AI
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# AI-005: ループ範囲がない曲で export_song の range に "loop" を指定すると、曲全体を書き出し、そのことを知らせない

## Where

crates/meloet-ai/src/tools/files.rs:480-482 (`RangeArg::Loop => doc.loop_range.or_else(|| TickRange::new(0, song_end))`)
crates/meloet-ai/src/tools/files.rs:50-51 (引数の説明: 「ループの小節。」 "The loop's bars.")

## What happens

ループ範囲がない曲で `export_song {range: "loop"}` を実行すると、曲全体 (最初から最後のクリップの終わりまで) を書き出し、「Wrote 1 file(s), N s」 と答えます。ループがなかったことには何も触れません。AI は、ファイルにループの小節が入っていると思ってしまいます。

## What should happen

AI は、頼まれたことができないときにははっきり知らされるべきです (作業の重点: 「AI に伝えるエラーははっきりさせる」)。同じ仲間の `set_loop_on` は、同じ場合に拒否します (「the song has no loop range; set one with set_loop」。timing.rs:294-296)。

## How to trigger

コードを読んだ結果です。実行はしていません。ノートのある新しい曲で、ループを設定しない。`export_song {"folder": "/tmp", "range": "loop"}` を呼ぶ。

## Cause

フォールバック `or_else(|| TickRange::new(0, song_end))` が、ループがないことを、曲全体へ黙って置き換えています。

## Suggested fix

`RangeArg::Loop` で`doc.loop_range == None` のとき、`set_loop_on` と同じように `failed("the song has no loop range; set one with set_loop, or give range \"song\"")` で答える。フォールバックを残したいなら、それを答えの文とこの引数の説明に書く。プロトコルの変更はありません。

## Verification

コードを読んで確認しました。`export_song` (`crates/meloet-ai/src/tools/files.rs:480-482`): `RangeArg::Loop => doc.loop_range.or_else(|| TickRange::new(0, song_end))`。ループ範囲がないと、曲全体が書き出され、答えはそれを言いません。引数の説明は 「ループの小節。」 と書いています。同じ仲間の `set_loop_on` は、同じ場合に拒否します (「the song has no loop range; set one with set_loop」)。重大度 `low` はそのままです。

修正の計画: `crates/meloet-ai/src/tools/files.rs`: `RangeArg::Loop` で `loop_range` がないとき、`failed("the song has no loop range; set one with set_loop, or give range \"song\"")`。フォールバックに頼っているテストが `crates/meloet-ai/tests/` にないか確認し、あれば一緒に変える。`docs/development/ai-tools.md` の `export_song` のエラーに一行加える。

## Fix

`crates/meloet-ai/src/tools/files.rs` (`export_song`): `range: "loop"` でループ範囲がないとき、`failed("the song has no loop range; set one with set_loop, or give range \"song\"")` と答え、ファイルは何も書かない。`docs/development/ai-tools.md`: `export_song` の行を一行加える。

テスト: `crates/meloet-ai/tests/link.rs` の `the_export_writes_over_a_file_only_when_asked` に、その場合を加えた (拒否され、答えの文に "no loop range" があり、ファイルは作られない)。
