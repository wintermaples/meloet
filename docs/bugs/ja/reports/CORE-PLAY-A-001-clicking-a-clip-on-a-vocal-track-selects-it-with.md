---
id: CORE-PLAY-A-001
title: 歌のトラックのクリップをクリックすると、歌のトラックの規則を使わずに選ばれ、キーの送り先が失われ、トラックに録音の●が付かない
area: CORE-PLAY-A
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-PLAY-A-001: 歌のトラックのクリップをクリックすると、歌のトラックの規則を使わずに選ばれ、キーの送り先が失われ、トラックに録音の●が付かない

## Where

- crates/meloet-core/src/play.rs:1070-1079 (`Play::click`: `session.play_target` を直接設定している)
- crates/meloet-core/src/play.rs:1061-1064 (`select_clip`) と 1096-1097 (1 つだけのクリップを選ぶ `select_clips`) が `click` を呼ぶ
- crates/meloet-core/src/play/vocal.rs:203-216 (`selected_track_changed`。飛ばされている規則)
- 同じ原因だが頻度は低い: crates/meloet-core/src/play/tracks.rs:24-27 (`delete_track` が選択を動かす) と crates/meloet-core/src/play.rs:1414-1417 (`after_change` が選択を先頭のトラックへ動かす) も、`selected_track_changed` を呼ばずに `play_target` を設定している。

## What happens

歌のトラックのオーディオクリップをクリックすると (`Command::SelectClip`、または `add`/`toggle` 付きで 1 つだけのクリップを選ぶ `SelectClips`)、そのトラックが選ばれたトラックになるが、次のことが起きない。
- `session.last_keys` に、それまで選ばれていたキーのトラックが入らない。そのため `Song::keys_target()` は `last_keys` にフォールバックし、その値は `None` (または以前のキーのトラック) になる。入力計の「弾くと鳴る先」は「—」(または誤ったトラック) を表示し、人はクリップをクリックしただけなのに鍵盤が鳴らなくなる。
- 歌のトラックに ● が付かない (`armed_vocals` が変わらない)。そのため R で声を録音できない (キーの送り先もないので、そもそも録音できない)。同じトラックのヘッダーをクリックする (`SelectTrack`) と、この両方が行われる。
- `SelectTrack` が入力のない歌のトラックに出す `NoInput` の知らせも出ない (state.rs 1857)。

## What should happen

仕様 3 (3-bands.md 99 行目): 「クリップを押すと、そのトラックも選ばれる。選ばれたトラックが「弾くと鳴る先」になる。」 仕様 3 の 256 行目 (94-1、35-A): 「歌のトラックが選ばれている間、キーボードは最後に選ばれたキーのトラックで鳴り続ける。」 アーキテクチャ 22-15 と 24-C: 選ばれた歌のトラックには ● が付く (`selected_track_changed` の doc コメント「a vocal track that is selected gets its ●」)。クリップでトラックを選ぶときも、ヘッダーでトラックを選ぶとき (`Play::select_track`、play.rs 1043-1052) と同じ規則に従うべきである。`open_piano_roll` (clips.rs 73-76)、`make_copy` (tracks.rs 126-129)、`place_pattern` (patterns.rs 150-154) はすでにそうしている。フェイク (`ui/src/bridge/fake.ts` の `settleSong`) も、`selectClip` が歌のトラックを選ぶとき、キーを前のキーのトラックに残す。

## How to trigger

実行して確かめたのではなく、コードを読んで確かめた。
1. キーのトラック 1 と、オーディオクリップのある歌のトラック 2 を含む、保存済みの曲を開く (`last_keys` と `armed_vocals` は保存されないので、どちらも最初は空になる)。トラック 1 のヘッダーをクリックして、トラック 1 でキーが鳴るようにする。
2. アレンジの表示で、トラック 2 のオーディオクリップをクリックする。
3. `song.keysTarget` が `null` になる。キーを押しても何も鳴らず、入力計は「—」を表示する。トラック 2 の ● は点かないので、何も録音できない (`TransportView::can_record` は false になり、R は `NoArmedTrack` を返す)。
トラック 2 のヘッダーをクリックすると、キーはトラック 1 に残り (`last_keys = 1`)、トラック 2 の ● が点く。`last_keys` に、以前の歌のトラックの選択で入った古いキーのトラックが残っているときは、キーはトラック 1 ではなくその古いトラックへ行く。

## Cause

`Play::click` は `self.song.session.play_target = Some(track.id)` を書く (play.rs 1073)。そして `selected_track_changed(before)` を一度も呼ばない。この関数は、新しく選ばれた歌のトラックについて `last_keys` を設定し `armed_vocals` に追加する、唯一の場所である。`select_track` はこの関数を呼ぶが、クリップの経路は呼ばない。

## Suggested fix

`Play::click` で、トラックが変わったときは `select_track` と同じ経路を使う: `let before = self.song.session.play_target.replace(track.id); if before != Some(track.id) { self.selected_track_changed(before); }`。`delete_track` と `after_change` のフォールバックにも同じことを入れるかどうかは検討する。クリックで `NoInput` の知らせも出すなら、`SelectTrack` の後に `state.rs` が `selected_vocal()` を調べるのと同じように、`SelectClip` の後にも調べる (ここはリードが決める)。プロトコル、ファイル、文言は変えない。

## Verification

コードを読んで確かめた (テストは実行していない)。`Play::click` (`play.rs:1070-1079`) は `session.play_target = Some(track.id)` を書くだけで、ほかには何もしない。`selected_track_changed` (`play/vocal.rs:203-216`) は、`last_keys` を設定して `armed_vocals` に追加する唯一の場所だが、呼び出し元は `select_track`、`open_piano_roll` (`clips.rs:75`)、`make_copy` (`tracks.rs:129`)、`place_pattern` (`patterns.rs:153`)、`vocal.rs:245` であり、`click` ではない。画面は、クリップを押したときに `selectClip` だけを送る (`Arrange.svelte:1378-1434`)。`selectTrack` は一緒に送らない。`last_keys` が空のままなので、クリック後の `Song::keys_target()` は `None` になり、キーはどこでも鳴らず、歌のトラックに ● も付かない。重大度 `medium` は変えない。

訂正: キーのトラックからクリップのクリックで歌のトラックへ選択が移るとき、`last_keys` も消えない。そのため「What happens」の 2 文目は、書かれたとおり正しい (古いキーのトラックが送り先になりうる)。

修正の計画: `crates/meloet-core/src/play.rs` の `Play::click`: `let before = self.song.session.play_target.replace(track.id); if before != Some(track.id) { self.selected_track_changed(before); }` を `!self.busy()` の分岐の中に入れる。`delete_track` (`play/tracks.rs:24-27`) と `after_change` (`play.rs:1414-1417`) で選択が動く箇所も、同じ 2 行で、テストが歌のトラックに当たることを示すなら入れる (同じ規則なので、テストで示せる範囲だけを変える)。クリックでの `NoInput` の知らせは、`state.rs:1857` で `SelectTrack` が出すのと同じ方法にする (既存の文言を使い、新しい文言は作らない)。テストは `crates/meloet-core/src/play/clips_tests.rs` か `tests.rs` に置く。歌のトラックのクリップを `SelectClip` した後、`keys_target` は前に選ばれていたキーのトラックになり、歌のトラックは armed になる。

## Fix

`crates/meloet-core/src/play.rs` の `Play::click` (クリップがクリックされたとき、`SelectClip` と 1 つだけのクリップを選ぶ `SelectClips`): 選ばれたトラックが変わったら、トラックのヘッダーをクリックしたときと同じく `selected_track_changed(before)` を実行する。選択がほかの理由で動く場合にも同じ規則を当てる。`play/tracks.rs` の `delete_track` (削除されたトラックを前のトラックとし、ステップの前の曲に対して実行する) と、`play.rs` の `after_change` で先頭のトラックへ切りかえる箇所 (前のトラックはなし。そのトラックは消えている)。`crates/meloet-core/src/state.rs`: `SelectClip` と `SelectClips` は、クリックで歌のトラックが選ばれ、入力がないときに、既存の `NoInput` の知らせを出す (`no_input_if_vocal_selected`)。

テスト (`play/clips_tests.rs` の中): `a_click_on_a_vocal_clip_selects_its_track_as_its_head_does`、`a_vocal_track_selected_in_place_of_a_deleted_one_gets_its_arm`、`a_vocal_track_selected_when_undo_takes_the_selected_one_away_gets_its_arm`。
