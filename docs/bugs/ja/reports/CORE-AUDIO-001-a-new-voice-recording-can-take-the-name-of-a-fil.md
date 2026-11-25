---
id: CORE-AUDIO-001
title: 新しい歌の録音が、曲のフォルダにすでにあるファイルと同じ名前をつけることがある。曲はそのファイルを再生して保存し、録音は失われる
area: CORE-AUDIO
kind: bug
severity: high
status: fixed
found: 2026-11-25
---

# CORE-AUDIO-001: 新しい歌の録音が、曲のフォルダにすでにあるファイルと同じ名前をつけることがある。曲はそのファイルを再生して保存し、録音は失われる

## Where

crates/meloet-core/src/service/input.rs:213-223 (`start_vocals`: 名前が空いているかは `!dir.join(name).exists()` で見る。`dir` は作業場所だけ)
crates/meloet-core/src/audiofiles.rs:113-125 (`Places::folders` / `Places::find`: 作業場所より前に、曲の `音/` と前の場所のフォルダを見る)
crates/meloet-core/src/audiofiles.rs:363-366 (`settle_until`: `<song>/音/` にすでにある名前は、中身が何であっても動かさない)
crates/meloet-core/src/service/input.rs:621-624 (`keep_vocals`: 復旧したファイルの名前がすでに表にあると、そのファイルは飛ばされる)

## What happens

録音したファイルは `<track name> <audio id>.wav` という名前になる (`vocalrec::file_name`)。`start_vocals` が確かめるのは、作業場所 (`work/音/`) にその名前のファイルがないかだけ。曲の `<song>/音/` (「別名で保存」のあとは前の場所のフォルダも) や、曲の音声の表にあって実ファイルがない名前は見ない。

曲のフォルダに別の同名ファイルがすでにある場合 (人が前に取りこんだ音声ファイルを、保存で移したもの)、録音は `work/音/<name>` に書かれ、新しい uid をもらう。しかし次のことが起きる:

- `AudioStore::find` は曲のフォルダを先に見るので、新しい uid の `PcmStore::open` は古いファイルを開く。新しいクリップは別のファイルの音を再生する (波形は録音の書き込み中のピークから出るので、録音のもの)。
- 次の保存で、`settle_until` は `<song>/音/<name>` がすでにあるのを見て、録音を動かさない。曲ファイルは両方のエントリに古いファイルの名前を書く。
- 曲を閉じるか別の曲を開くと、作業場所は空になり (`clear_work`)、録音は完全に失われる。
- そのような録音の途中で meloet が終わると、`keep_vocals` は名前が表にあるのを見て、ファイルを残さない。

表にその名前のエントリがあり、ファイルが欠けている場合も同じことが起きる (たとえば、人が別の場所へ動かした取りこみファイル)。古いエントリは録音を「見つけて」、それを再生する。

## What should happen

作業場所で新しいファイルに名前をつける他のすべての場所は、曲が見るすべての場所と、表の名前を確かめる。`AudioStore::bring_in` (audiofiles.rs:294-313、`self.find(&name)` と `taken`) と `clips::put_in_place` (audiofiles/clips.rs:273、`taken.contains(&name) || store.find(&name).is_some()`) がそうである。docs/architecture.md 22-4 は「同じ名前の別のファイルは « 2» をつける」、22-16 は「使われている名前は « (2)» をつける」と書く。録音が、別のファイルに隠されたり、別のファイルとして保存されたりすることは決してあってはならない。

## How to trigger

実行せず、コードを読んで確かめた。

1. 保存済みの曲 B で、トラック「歌」(歌のトラック) に、別の meloet の曲の録音と同じ名前の wav ファイルを取りこむ。例として、曲 A の `音/` にある `歌 40.wav`。曲 B の次の id より大きい番号にする。保存すると、ファイルは `B/音/歌 40.wav` に動く。
2. トラック「歌」で録音を続け、ある録音が音声 id 40 をもらうまで続ける (id は、録音・クリップ・テイクごとに数個ずつ増える。新しい曲は 4 から始まる)。
3. そのテイクは、声ではなく取りこんだファイルを再生する。保存して、曲を閉じ、もう一度開くと、テイクの声は消えている。

## Cause

`start_vocals` は `.find(|name| !names.contains(name) && !dir.join(name).exists())` で名前を選ぶ。ここで `dir` は `work_audio_dir(state_dir)`。`AudioStore::find` (曲の `音/` と前の場所も見る) を使わず、表の名前 (`taken`) も受け取らない。名前の中の id は曲にとって新しいが、取りこんだファイルは前の名前を保つので、どの `<name> <number>.wav` も持ちうる。

## Suggested fix

`start_vocals` で、`self.audio_store.find(name).is_some()` のとき、または曲の音声の表にそのファイル名がすでにあるとき (取りこみが使う `taken` の一覧)、その名前は使用中とみなす。今と同じく « (2)»、« (3)» へ進む。プロトコル、ファイルの形式、文言は変えない。

## Verification

テスト (別の worktree、`cargo test -p meloet-core --features dev-hooks --lib verify_core_audio_001`) で確認した。ただし一つの手順はコードを読んで得た。テストの内容: `AudioStore` に、`音/` に「歌 40.wav」 (前に取りこんで保存したファイル) がある曲フォルダと、空の作業場所を用意する。`start_vocals` の規則 (`service/input.rs:219-223`、作業場所だけの `!dir.join(name).exists()`) では、`vocalrec::file_name("歌", 40)` = 「歌 40.wav」 は空いている。録音は `work/音/歌 40.wav` に書かれる。次に、実際のストアで動かす: `AudioStore::find("歌 40.wav")` は曲フォルダのファイルを返す (中身は「取りこんだファイル」)。そのため新しいクリップは別のファイルを再生する。`settle` (保存) は録音を動かさない。名前がすでに `音/` にあるからだ。`clear_work` (曲が手放す) は録音を消す。その後に残る「歌 40.wav」は取りこんだファイルだけで、録音は失われる。テストでは `start_vocals` 自体は呼んでいない (サービス全体と入力ストリームが要るため)。名前の規則は上に引用した二行で、`Play::vocal_file_asks` は `<track name> <audio id>.wav` を返し、表は見ない。重大度 `high` は変えない (録音が失われ、それまでは別のファイルが代わりに再生される)。ただし、名前が `<track name> <a later audio id>.wav` の取りこみファイルが必要。たとえば、別の meloet の曲から引き継いだテイク。

訂正: なし。表の中のエントリでファイルが欠けている場合、および `keep_vocals` が表にある名前の復旧ファイルを飛ばす件 (`input.rs:621-624`) は、読んだだけで確かめていない。

修正案: `crates/meloet-core/src/service/input.rs` の `start_vocals`: 名前は、`self.audio_store.find(name).is_some()` (曲が見る全部の場所) のとき、または曲の音声の表にそのファイル名があるときにも使用中とする。今と同じく « (2)» へ進む。表の名前は要求と一緒に渡す必要がある: `crates/meloet-core/src/state.rs` の `Effect::StartVocalRecording` に `taken: Vec<String>` (`doc.audio` のファイル名。取りこみが `taken` を渡すのと同じ)を加え、`play.rs:857` で埋める。テスト: `state/vocal_tests.rs` (効果が表の名前を運ぶこと)、および名前の選び方のサービス級かストア級のテスト (小さな自由関数 `free_recording_name(store, dir, taken, names, ask)` にすると、入力ストリームなしで試せる)。

## Fix

`crates/meloet-core/src/service/input.rs`: 録音のファイル名は、新しい `free_recording_names` が決める。名前は、作業場所にそのファイルがあるとき (前と同じ)、曲が見る場所のどれかにあるとき (`AudioStore::find`: 曲の `音/`、前の場所)、または曲の音声の表がその名前を持つとき、使用中とする。その後は « (2)»、« (3)»、…。`crates/meloet-core/src/state.rs` の `Effect::StartVocalRecording` が `taken` (`doc.audio` のファイル名) を運ぶ。`crates/meloet-core/src/play.rs` の `record` で埋め、`crates/meloet-core/src/service.rs` で渡す。テスト: `service::input::tests::a_recording_never_takes_a_name_the_song_has` (実際の曲フォルダを持つ実際の `AudioStore` での、実際の命名関数。修正がないと失敗する) と `state::vocal_tests::only_the_voice_is_recorded_when_there_is_no_keys_track_and_a_loop_makes_takes` (二回目の録音の効果が表の名前を運ぶ)。`state::vocal_tests::the_keys_and_the_voice_are_recorded_together_as_one_step` は `taken: []` を期待するように変えた。
