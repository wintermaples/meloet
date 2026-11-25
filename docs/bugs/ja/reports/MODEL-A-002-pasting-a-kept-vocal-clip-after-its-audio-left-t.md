---
id: MODEL-A-002
title: 音の表から歌のファイルが外れた後で、残しておいた歌のクリップを貼りつけると、無音のクリップと開けない曲のファイルができる
area: MODEL-A
kind: bug
severity: high
status: fixed
found: 2026-11-25
---

# MODEL-A-002: 音の表から歌のファイルが外れた後で、残しておいた歌のクリップを貼りつけると、無音のクリップと開けない曲のファイルができる

## Where

crates/meloet-model/src/arrange.rs:183-200 (`paste_clips`: 残しておいたクリップが `place` を通してそのまま入る)
crates/meloet-model/src/arrange.rs:261-270 (`place`: 新しい `Doc` は `doc.audio` をそのまま保つ)
crates/meloet-core/src/play.rs:1196-1199, 2093 (`clip_board` は取り消しをまたいで残る。曲が差し替えられたときだけ消える)
crates/meloet-core/src/songfile.rs:1307 and 1447-1456 (ファイルの音の表は `used_audio(doc)` で、`doc.audio` からだけ取る)
crates/meloet-core/src/songfile.rs:1627-1628 and 1501-1502 (`audio_take`: 表にないファイルを指す音のテイクがあると、曲は `None` になる)

## What happens

モード C は `Arc<Clip>`（`KeptClip`）を保持します。歌のクリップのテイクは、`Arc<AudioSource>` を直接持っています。コピーの後で、そのファイルを音の表（`Doc::audio`）に入れた手を取り消すと（「音のファイルを入れる…」（`import_audio`）、歌の録音、「逆にする」）、表からそのファイルがなくなりますが、クリップ・ボードにはまだクリップが残っています。モード V（または歌のレーンの「貼りつけ」）は、そのあと、ファイルが `Doc::audio` にない歌のクリップを曲に入れます。
- エンジンに伝わるのは表だけです（`EngineOp::AudioSources { sources: doc.audio }`、play/audiosync.rs:110-113）。その表にはそのファイルがありません。
- 保存すると、クリップの音のテイクは `audio: <id>` で書かれます。しかし、ファイルの表は `used_audio(doc)` で、これは `doc.audio` の絞り込みです。そのため、ファイルはその表に入りません。曲をもう一度開くと、`audio.get(&t.audio)?` で失敗し、曲全体が `ReadError::Broken` になります。なおすためのファイルも同じ書き方を使うので、こわれています。

## What should happen

`Doc::audio` は「曲の音のテイクが再生するすべての音のファイル」です（crates/meloet-model/src/song.rs:368）。音のテイクを曲に入れる他の手は、どれも同じ手の中でファイルも表に入れています（`import_audio`、audio/clips.rs:301; `add_vocal_recordings`、audio/record.rs:215; `reverse_clips`、audio/reverse.rs:260）。アーキテクチャ 17（貼りつけ: Rust がクリップを覚えておき、テイクは `Arc` でだけ共有する）は、貼りつけが、クリップがもともと持っていたものと同じ曲を作ることを求めています。

## How to trigger

コードを読んで確認したもので、実際には動かしていません。
1. 歌のトラックで「音のファイルを入れる…」により WAV ファイルを入れます（一つの手で、ファイルは `Doc::audio` に入ります）。
2. 新しいクリップをクリックし、モード C を押します。
3. モード Z を押します（入れた手が取り消され、表は入れる前に戻ります）。
4. 歌のトラックを選び、モード V を押します。
5. 保存し、閉じ、もう一度開きます。曲は、こわれているとして開けません。（歌の録音を入れた場合も同じです。）

## Cause

`paste_clips`（`place` を通して）はクリップを置くだけで、`Doc::audio` を見ることはありません。キーのテイクは表を必要としないので、キーのクリップではこれで正しいです。しかし、残しておいた歌のクリップは、表にあるファイルの登録より長く残ることがあります。`History::undo` がファイルを表に入れていない `Doc` を戻す一方、コアのクリップ・ボードはそのクリップを残すためです。

## Suggested fix

`paste_clips` で、置かれた歌のクリップのそれぞれについて、`doc.audio` にない（`AudioId` で比べる）`take.source` を、表のコピーに（id の順で）足し、同じ手の中で `new.audio` に設定します。そのファイルの id は同じ `next_id` で作られ、二度と作られないので、ぶつかりません。プロトコルやファイル形式の変更はありません。テスト: 取り込み、コピー、取り消し、貼りつけの後、`songfile::decode(songfile::encode(..))` が曲を読み直し、`Doc::audio` にファイルがあること。

## Verification

テストを実行して確認しました（作業用の worktree で `cargo test -p meloet-core --features dev-hooks --lib verify_model_a_002`）。`songfile/vocal_tests.rs` の歌の曲から、一つの `History` の手でクリップ 43 とそのファイル（音 51）を入れ、クリップ 43 を `arrange::copy_clips` でコピーし、`History::undo`、歌のトラックで `arrange::paste_clips` を行いました。貼りつけた曲の音の表は `[AudioId(50)]` だけで、`songfile::decode(&songfile::encode(..))` は `Err(Broken)` を返しました。コアは取り消しをまたいで `clip_board` を保ち（`play.rs:1198`、曲が差し替えられたとき `play.rs:2093` でだけ消える）、`Play::paste_clips`（`play.rs:1245`）には独自の確認がありません。重大度 `high` のままです。

訂正はありません。エンジン側も書いたとおりです。`EngineOp::AudioSources` は `doc.audio` だけを運ぶので、保存するまで貼りつけたクリップは無音です。

修正の計画: `crates/meloet-model/src/arrange.rs` の `paste_clips`（`place` の後）で、残しておいた歌のクリップのそれぞれについて、`AudioId` が `doc.audio` にない `take.source` を、表のコピー（読み込み側と同じく id の順）に足し、同じ手の中で新しい `Doc` に設定します。テスト: `crates/meloet-model/src/arrange/tests.rs`（貼りつけの後、表にファイルがある）と `crates/meloet-core/src/songfile/vocal_tests.rs`（取り込み、コピー、取り消し、貼りつけ、`decode(encode(..))`）。ディスク上のファイルは、修正しながら確認します。取り消した録音または取り込みを貼りつけた後も、ファイルが `audiosync` の見る場所にあるかどうか（取り消した手のファイルはやりなおしのために残るので、あるはず）。この部分は実行していません。

## Fix

`crates/meloet-model/src/arrange.rs` の `paste_clips`: 残しておいた歌のクリップが鳴らすファイルのうち、`Doc::audio` にないものは、（id の順で）表のコピーに戻し、貼りつけた `Doc` に同じ手の中で設定します。何も足りなければ、表は同じ `Arc` のままです。コアの `sync_audio_sources` は、新しい表を見て、ファイルを入れた `EngineOp::AudioSources` を送ります。貼りつけを取り消すと、そのファイルを除いた表を送ります。ファイル自体は、曲を開いている間は作業場所か曲のフォルダーに残るので（アーキテクチャ 22: メモリ上の表は使わなくなったファイルも保ち、作業場所は曲を手放すときにだけ消す）、ストアはそれを見つけます。`docs/architecture.md` の 17（貼りつけ）にその旨を書きました。

テスト: `arrange::tests::a_pasted_vocal_clip_brings_its_file_back_into_the_audio_table`（meloet-model）と `songfile::vocal_tests::a_vocal_clip_pasted_after_its_import_was_undone_keeps_its_file`（meloet-core: `Play` で取り込み、コピー、取り消し、貼りつけ。エンジンはファイルのある表を受け取る。`decode(encode(..))` が曲を返す。取り消すと、そのファイルを除いた表を送る）。
