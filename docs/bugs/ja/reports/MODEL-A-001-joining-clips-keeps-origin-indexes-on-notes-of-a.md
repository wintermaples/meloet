---
id: MODEL-A-001
title: クリップをつなげると、元がない テイクのノートに元の番号が残り、保存した曲を開けなくなる
area: MODEL-A
kind: bug
severity: high
status: fixed
found: 2026-11-25
---

# MODEL-A-001: クリップをつなげると、元がない テイクのノートに元の番号が残り、保存した曲を開けなくなる

## Where

crates/meloet-model/src/arrange.rs:574-581 (`join_clips`: `notes.push(Note { start, length, ..*n })` は、元のテイクの `n.from` をそのまま残す)
crates/meloet-model/src/arrange.rs:595-601 (つなげたテイクは `origin: None` で作られる)
crates/meloet-core/src/songfile.rs:1540-1543 (`take_file` は、`from` が `Some` のノートすべてを `NoteFile::From` として書く)
crates/meloet-core/src/songfile.rs:1896-1907 (`to_take`: 元がないテイクの `From` のノートがあると、曲全体が `None` になる)

## What happens

つなげるクリップのひとつが量子化されたテイクを表示しているとき、つなげたテイクには元がない (`origin: None`) のに、そのノートには古い量子化テイクの元の番号 (`from: Some(i)`) が残ったままになります。曲は普通に再生でき、編集もできます。しかし保存すると (または 2 秒後に復元ファイルを書くと)、そのようなノートはそれぞれ `NoteFile::From(start, length, key, velocity, i)` として書かれます。そのファイルをもう一度開くと失敗します。`to_take` は、`i < from_len` のときだけ `From` のノートを受け入れます。元がないテイクでは `from_len` が 0 なので、`to_song` は `None` を返し、曲は `ReadError::Broken` になります。復元ファイルも同じ `songfile::decode` で読むため、曲は戻りません。使う人から見れば、曲全体が失われます。

## What should happen

`Note::from` は、「書いた、コピーした、貼りつけたノート、および元がないテイクのノートでは `None`」です (crates/meloet-model/src/song.rs:33-35)。アーキテクチャ 24-6 は、つなげたテイクは 「他のテイクと量子化の元は残さない」 と書いています。元を捨てる同じ仲間の処理は、番号もきちんと消しています。`unquantize` は元を捨てるとき `n.from = None` にし (quantize.rs:164-168)、`overdub_recording` はクリップを合わせて元を捨てるとき `from: None` にしています (record/overdub.rs:104-110)。

## How to trigger

コードを読んだ結果です。実行はしていません。
1. クリップ A に録音したノートを入れ、Q (量子化) を押す。表示しているテイクには元があり、そのノートには `from` が付く。
2. 同じトラックで A の直後に、鍵のクリップ B を置く。
3. A と B を選び、「クリップをつなげる」 (`Command::JoinClips`。AI の連携からのつなげる操作も同じ) を行う。
4. 保存して閉じ、もう一度開く。曲は壊れているとして開けない (クラッシュ後の復元ファイルも同じ)。

## Cause

`join_clips` は、新しいテイクの各ノートを `..*n` で作るため、つなげる各クリップの表示中のテイクから `from` をコピーします。一方で、新しいテイクは `origin: None` で作られます。モデルは `from` が元のもとでだけ付くかを確認していません。しかし曲ファイルの読み込みはそれを確認し、曲全体を開くのを拒みます。

## Suggested fix

`join_clips` で、つなげたテイクに入れる各ノートに `from: None` を書きます (`overdub_recording` が、合わせたテイクでそうしているのと同じです)。プロトコルやファイルの形は変えません。テスト: 量子化したクリップを隣と連結し、つなげたテイクのどのノートにも `from` がないこと、そして `songfile::decode(songfile::encode(..))` で曲が読み戻せることを確認します。

## Verification

テストを実行して確認しました (作業用の worktree。`cargo test -p meloet-core --features dev-hooks --lib verify_model_a_001`)。新しい曲に鍵のクリップを 2 つ隣り合わせて置き、最初のクリップに `quantize::quantize`、両方に `arrange::join_clips` をかけ、`songfile::decode(&songfile::encode(..))` を実行しました。つなげる前は曲を読み戻せます。つなげた後は、つなげたテイクが `origin: None` になり、ノートが `from: [Some(1), None]` になります。そして `decode` は `Err(Broken)` を返します。復元ファイルも同じ `decode` を通るため、同じく失われます。重大度 `high` はそのままです (保存した曲を開けなくなるため)。

Corrections: 訂正なし。行の記載も原因も、書かれたとおりです。両方の呼び出し元 (`play/clips.rs:40` と、AI 連携の `play/ai.rs:377`) は、同じ `arrange::join_clips` を通ります。

修正の計画: `crates/meloet-model/src/arrange.rs` の `join_clips` で、つなげたテイクに入れる `Note` に `from: None` を書く。テスト: `crates/meloet-model/src/arrange/tests.rs` (つなげたテイクのどのノートにも `from` がない)、`crates/meloet-core/src/songfile/tests.rs` (量子化、つなげ、`decode(encode(..))` で曲が戻る)。この方法で書かれた曲ファイルは、そのままでは開けないままです。元がないテイクの `From` のノートを許して平のノートとして読む (そうすれば既に書かれたファイルも開けるようになる) こともできますが、これはリードの判断であり、修正には必要ありません。

## Fix

`crates/meloet-model/src/arrange.rs` の `join_clips`: つなげたテイクに入れる各ノートに `from: None` を付ける。`crates/meloet-core/src/songfile.rs` の `to_take` (リードの判断): 元がないテイクにある、元の番号を持つノートは、平のノートとして読む (番号は捨てる)。そのため、この不具合で既に書かれた曲や復元ファイルも、もう一度開けるようになる。元があるテイクで、元のノートの数を超える番号を持つものは、これまでどおり読めない。`docs/architecture.md` 14 にそのことを書く。

テスト: `arrange::tests::joining_a_quantized_clip_keeps_no_origin_indexes` (meloet-model)、`songfile::tests::a_joined_quantized_clip_comes_back` と `songfile::tests::an_origin_note_in_a_take_with_no_origin_reads_as_plain` (meloet-core)。変更: `songfile::tests::origins_that_cannot_be_do_not_read` は、「元がないテイクにある元のノート」を壊れているとして数えなくなった。
