---
id: UI-TEXT-006
title: 「このトラックを音にする」で作られたトラックの名前が、英語の画面で日本語の接尾語を付けられる
area: UI-TEXT
kind: parity
severity: low
status: fixed
found: 2026-11-25
---

# UI-TEXT-006: 「このトラックを音にする」で作られたトラックの名前が、英語の画面で日本語の接尾語を付けられる

## Where

crates/meloet-model/src/audio/bounce.rs:17 (`BOUNCED_SUFFIX = "（音）"`)
crates/meloet-model/src/audio/bounce.rs:158-168 (`bounced_name`)

## What happens

ページを英語にして、「Piano」という名前のキーのトラックに対して「このトラックを音にする」("Turn this track into audio") を実行すると、新しい歌のトラックが「Piano（音）」という名前になる(塞がっていれば「Piano（音） 2」)。英語の画面に日本語の語と全角のかっこが出る。表示されるのは、トラックの見出し、ミキサーの欄、取り消しの履歴の行、状態行の「Put audio clips on 2 Piano（音）…」である。

## What should happen

architecture 25-36「Rust で作る名前」: 名前を作る場所で言語を渡す(`vocal_track_name`、`InstrumentRef::track_name` など)。`SetLanguage` の後は、以後作られるものがそれに従う。仕様書 7-readings D4「新しいものの名前」は、meloet が作る各名前の英語を与える。変換されたトラックの名前は、新しい歌のトラックの「Vocal」と同じく、その時点で meloet が作る。日本語のままでよいもの(曲の中のフォルダ、ファイル名、ログ、AI の連携)の一覧に、これは入っていない。

## How to trigger

コードから読んだもので、実行はしていない。「表示 ▸ ことば / Language ▸ English」を選ぶ。クリップのあるキーのトラックで、右クリック ▸「Turn this track into audio」を選ぶ。終わるまで待つ。

## Cause

`bounce_track` は固定の `BOUNCED_SUFFIX` でトラックに名前を付け、ラウンド 12 D4 の他の名前を作る処理と違って `Language` を受け取らない。

## Suggested fix

`bounce_track` に `meloet_model::Language` を渡し (`Play::names` から、`add_vocal_track` が受け取るのと同じように)、接尾語は `language.pick("（音）", " (audio)")` で選ぶ。または、使う人が承認する別の英語を使う。新しい英語の文面が要る(仕様書 3 にあるのは 「（元の名前）（音）」 だけ。仕様書 7-readings D4 は承認された英語の名前を並べているので、リードかオーナーが選ぶ)。プロトコルや曲のファイルの変更はない。作業ファイルの名前 (`bounce_file_name`、コアの play/bounce.rs:71) はトラックの名前に従うので、そのままでよい(ファイル名は D4 の対象外)。

## Verification

実在する(読んだもので、実行はしていない): `bounce_track` は新しいトラックに固定の `BOUNCED_SUFFIX = "（音）"` で名前を付け (`crates/meloet-model/src/audio/bounce.rs:17`、158-168)、`Language` を受け取らない。一方 architecture 25-36「Rust で作る名前」は、meloet が名前を作る場所ならどこでも言語を渡す(`vocal_track_name`、`add_vocal_track`、`import_audio` …)。その一覧 (曲の中のフォルダ、ファイル名、ログ、AI の連携) にこの名前は入っていない。したがって英語のページでは「Piano（音）」になる。

`needs-lead`: 英語の接尾語は新しい文面だから。既存の文面を探した: 仕様書 3 (A7) にあるのは 「（元の名前）（音）」(塞がっていれば 「ピアノ（音） 2」) だけ。仕様書 7-readings D4「新しいものの名前」は承認された英語の名前を並べているが、これにはない。`ui/src/text/en.ts` には操作の文言 ("Turn this track into audio"、`menu.bounceTrack`、"Turn into audio clips") はあるが、名前の形はない。Rust の名前は、どのみち `ui/src/text/` にはない(モデルの中で `language.pick` によって選ばれる)。

推奨する文面: 日本語は 「（音）」 のまま。英語は " (audio)"(空白の後に半角のかっこ)。よって「Piano (audio)」、塞がっていれば「Piano (audio) 2」。語の出どころ: "audio" は、この機能で 音 に使われる語として `en.ts` と `docs/spec/glossary.md` が使う語である ("Turn this track into audio"、"audio clips")。かっこの形は `msg.recordingVocal` の "{track} ({input})" と同じ。

選択肢: (a) `Language` を `bounce_track` に渡し (`Play::names` から、`add_vocal_track` が受け取るのと同じように)、接尾語を選ぶ。名前を 7-readings D4「新しいものの名前」に足す。`bounced_name` のテストを両方の言語で書く。プロトコルや曲のファイルの変更はない。古い曲は名前を保つ(名前はひとりでに変わらない)。(b) 両方の言語で 「（音）」 のままにし、architecture 25-36 の「変えない」の一覧に足す。私は (a) を勧める。作業ファイルの名前 (`bounce_file_name`、`crates/meloet-core/src/play/bounce.rs`) はトラックの名前に従うので、変更は要らない。

## Fix

リードの判断 (a): `bounce_track` は `Language` を受け取る (`crates/meloet-model/src/audio/bounce.rs`。`bounced_name` は `BOUNCED_SUFFIX` の 「（音）」 か " (audio)" を選ぶ。塞がっているときの規則は同じ)。`crates/meloet-core/src/play/bounce.rs` は `self.names` を渡す。保存済みの曲は名前を保つ。作業ファイルの名前 (`bounce_file_name`) は 「（音）」 のまま(ファイル名は D4 の対象外)。文書: architecture 25-36「Rust で作る名前」と A7 の行、7-readings D4 の行「音にしたトラックの名前」(リードが 2026-11-25 に決めた)。
テスト: `crates/meloet-model/src/audio/bounce/tests.rs` の `in_english_the_name_ends_in_audio_with_the_same_numbers` (テストの他の呼び出しは `Language::Ja` を渡す)。
