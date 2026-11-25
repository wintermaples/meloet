---
id: CORE-AUDIO-007
title: 「さがす…」で中身の違うファイルを選ぶと、クリップは古いデコード済みの複製を再生し、古い波形を表示し続ける
area: CORE-AUDIO
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-AUDIO-007: 「さがす…」で中身の違うファイルを選ぶと、クリップは古いデコード済みの複製を再生し、古い波形を表示し続ける

## Where

crates/meloet-core/src/audiofiles/keeper.rs:414-443 (`Work::found`: 同じレートとチャンネル数のファイルは、中身にかかわらず元のファイルの名前でコピーされる。古い中身のために保持していたものは何も手放さない)
crates/meloet-core/src/audiofiles.rs:246-262 (`cached` / `playable`: レートが合えば、開ける複製 `audio/<uid>-<rate>.wav` を準備済みとみなす)
crates/meloet-core/src/audiofiles.rs:429-439 (`PcmStore::open`: 再生されるのも同じ複製)
crates/meloet-core/src/audiofiles/keeper.rs:465-467 (`watch`: 波形のファイルは、フレーム数が `source.frames` と同じなら残す)

## What happens

「さがす…」は、レートとチャンネル数が合えば、SHA-256 の違うファイルも受け入れ (docs/architecture.md 22-18 A2, B-94-30)、作業の場所へ、見つからなかった元のファイルの名前でコピーする。元のファイルは `uid` を保ち、その uid で作られたものは、古いファイルから作ったものも含めて、すべてキャッシュに残る (表の元のファイルは、キャッシュの「使用中」を保つ):

- 見つかったファイルが素の wav でエンジンのレートになければ (mp3、flac、ogg、または別のレートの wav)、`playable` と `PcmStore::open` は古いデコード済みの複製 `audio/<uid>-<rate>.wav` を見つけて使う。クリップは見つかったファイルではなく古い音を鳴らし、見つかったファイルはデコードされない。
- 古い波形のファイル `peaks/<uid>.peaks` は `frames == source.frames` (表の古い数) を示すので、`watch` は波形を作り直さない。素の wav で新しい中身を再生する場合でも、波形は古いファイルのものになる。

保存すると、見つかったファイルは `音/` へ入る。別の PC では、またはキャッシュを空にした後では、同じ曲は見つかったファイルを再生する。つまり同じ曲が PC によって違う音になる。

## What should happen

docs/architecture.md 22-18 A2: 「見つかったファイルは作業の場所へ入り、次の保存で 音/ へ移る」。そのファイルが以後その曲の音声であり、再生も描画も、そこから来なければならない。22-8: デコード済みの複製は「作り直せる」。22-9: 「なくなったキャッシュのファイルは作り直す」。キャッシュはファイルの複製であり、二つ目の正しい元にはならない。

## How to trigger

コードから読んだもので、実行はしていない。

1. 曲に mp3 を入れて一度再生する(keeper が `~/.cache/meloet/audio/<uid>-48000.wav` にデコードし、波形を書く)。保存する。
2. meloet を閉じる。別のプログラムで mp3 を変える(たとえば小さくする、または真ん中の一部を切り出す)。同じレートとチャンネル数で別の名前で保存し、元のファイルは移動する。
3. 曲を開く: クリップがない。「さがす…」を使い、変えたファイルを選ぶ。状態行は見つかったと言うが、クリップは古い中身を再生し、描く。

## Cause

`Work::found` はファイルをコピーして `file_arrived` を呼ぶだけである。キャッシュは uid だけで探される (`cache_path(uid, rate)`, `peaks_path(uid)`)。取り込んだ音声の uid は古い中身のハッシュなので、古い複製は新しいファイルにも有効なままになる。

## Suggested fix

`Work::found` で、見つかったファイルの SHA-256 が `source.sha256` と違う場合 (または `source.sha256` が `None` の場合)、`file_arrived` の前に、その uid のデコード済みの複製 (`audio/<uid>-*.wav`) と波形のファイルを消す。そうすれば `watch` が見つかったファイルから作り直す。SHA-256 が同じなら、今のまま残す。プロトコル、ファイルの形、文面は変えない。(見つかったソースに新しい uid を与える方法もあるが、曲の表を変えることになる。)

## Verification

コードを読んで確認したもので、テストは実行していない。`Work::found` (`audiofiles/keeper.rs:414-443`) は、同じレートとチャンネル数のファイルを、見つからなかった元のファイルの名前で作業の場所へコピーし、`file_arrived` を呼ぶ。古い中身から作ったものは何も手放さない。`AudioStore::playable` と `PcmStore::open` (`audiofiles.rs:251-262, 429-439`) は、見つかったファイルが、エンジンのレートの素の wav でない限り、デコード済みの複製 `audio/<uid>-<rate>.wav` を使う。`watch` (`keeper.rs:465-467`) は、フレーム数が `source.frames` と同じである限り波形のファイルを残す。uid は変わらないので、古いデコード済みの複製が再生され、古い波形が描かれる。architecture 22-18 A2 は、SHA-256 の違うファイルを意図して受け入れる (「別のアプリで同じ曲を保存し直したかもしれない」)。22-8 / 22-9 はキャッシュを作り直せる複製と呼ぶ。重大度 `low` はそのままにする(「さがす…」で中身の違うファイルを選んだ後だけで、デコード済みの複製は、そのファイルがエンジンのレートの wav でないときだけ)。

訂正: なし。見つからない元のソースのキャッシュがいつ消されるかは確認していない。もう消えていれば、波形の半分だけが残る。

Fix plan: `crates/meloet-core/src/audiofiles/keeper.rs` の `Work::found`: コピーの後、見つかったファイルの SHA-256 が `source.sha256` でない (または無い) 場合、その uid のデコード済みの複製と波形のファイルを消し (キャッシュのパスを知っている唯一の場所である `AudioStore` の小さなメソッド)、uid を `peaks_made` から外してから `file_arrived` を呼ぶ。テスト `audiofiles/tests.rs`: 一つの uid のキャッシュの複製と波形がある状態で、別の中身の `found` を呼ぶ → 両方なくなり、`playable` が `NeedsCache` を返す。

## Fix

`crates/meloet-core/src/audiofiles/keeper.rs` の `Work::found`: コピーの後、見つかったファイルの SHA-256 が `source.sha256` でない (または無い) 場合、新しい `AudioStore::drop_cache` (`crates/meloet-core/src/audiofiles.rs`) がその uid のデコード済みの複製を、すべてのレートで、そして波形のファイルを消し、uid は `peaks_made` から外れる。その後 `watch` が見つかったファイルから作り直す。同じバイト列なら残す。テスト: `audiofiles::tests::a_found_file_with_other_contents_is_made_ready_again_from_itself` (修正がなければ失敗する)。
