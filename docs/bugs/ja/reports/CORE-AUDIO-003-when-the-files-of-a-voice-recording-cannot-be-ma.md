---
id: CORE-AUDIO-003
title: 声の録音のファイルが作れないとき、録音は声なしで続き、使う人に何も知らされない
area: CORE-AUDIO
kind: ui-ux
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-AUDIO-003: 声の録音のファイルが作れないとき、録音は声なしで続き、使う人に何も知らされない

## Where

crates/meloet-core/src/vocalrec.rs:591-615 (`Side::start`: `fs::create_dir_all`、`WavFile::create`、フォルダの同期。どれかが失敗すると開始が終わる)
crates/meloet-core/src/service/input.rs:257-263 (`start_vocals`: エラーは stderr に出るだけで、`Input::VocalsNotStarted` を返す)
crates/meloet-core/src/state.rs:1394 (`Input::VocalsNotStarted => self.play.vocals_not_started()`。知らせはない)
crates/meloet-core/src/play/vocal.rs:398-403 (`vocals_not_started`: 声のトラックを録音から外す)

## What happens

ディスクへの書き込みで、録音のファイルが作れないとき(ディスクに空きがない、作業の場所に書く権利がない、状態のフォルダがない、ファイル名をファイルシステムが受け付けない)、`start_vocals` は stderr に `meloet: cannot record into …` と書き、`VocalsNotStarted` を返す。ルールは声のトラックを録音から外し、録音は続く(エンジンにはすでに録音を指示している。play.rs:855-900)。何も見せない。状態行にも、ダイアログにも出ない。武装した声のトラックは、録音中にクリップを再生しない(`muted` に入る)。波形も伸びず、止めたあとも声のテイクはできない。歌を一回分ずっと歌った人は、あとで何も残っていなかったと知るだけで、理由は知らされない。

## What should happen

docs/bugs の指示: エラーを飲みこんで、使う人が気づかないままにしてはならない。ほかの部分は、録音がやるべきことをできないとき、使う人に知らせている。入力がない武装トラックのための `StatusMessage::NoInput` の知らせ(play.rs:838)と、録音中に音の仕組みや入力がなくなったときの仕様 6 の文言(「…録音を止めました。そこまでの演奏は テイク 2 に残っています。」)。docs/architecture.md 4-2 では、録音が続けられないとき「知らせを出す」とある。

## How to trigger

コードを読んで確かめたもので、実行はしていない。

1. 作業の場所を書けなくする(例: `chmod a-w ~/.local/state/meloet/work/音`)か、ディスクをいっぱいにする。
2. 入力のある声のトラックを武装し、録音を押す。
3. カウントインと録音は普段どおり動く。声が録音されていないことは、何も言わない。止めたあとも、声のテイクはない。

## Cause

`Input::VocalsNotStarted` には理由が入らず、その処理は `rec.vocal.tracks` を消すだけで、`StatusMessage` は何も設定しない。

## Suggested fix

state.rs の `Input::VocalsNotStarted` の処理で知らせを設定する(必要なら、声のトラックだけの録音なら止める)。新しい文言が要るので(仕様 6 にはない)、文面は作る側かオーナーさんが決める。既存の `StatusMessage` の形に新しい種類を足すなら、プロトコルは変えなくてよい(ただし、それはプロトコルの追加になる)。

## Verification

実際に起きることを、コードを読んで確かめた(テストは実行していない)。`Side::start`(`vocalrec.rs:591-615`)は、`create_dir_all`、`WavFile::create`、フォルダの同期のどれかで失敗する(入力が開いていないときも)。`start_vocals`(`service/input.rs:257-263`)は stderr に書き、`Input::VocalsNotStarted` を返す。その処理(`state.rs:1394`)は `Play::vocals_not_started`(`play/vocal.rs:398-403`)を呼ぶだけで、これは声のトラックを録音から外す。知らせはどこにも設定されず、録音は続く。

指示どおり、既存の文言を探した。`StatusMessage` には `NoInput`、`InputLost`(「…が抜けました。録音を止めました。…」)、`RecordingStoppedAudioSystem` / `RecordingStoppedOutputMissing`、`RecordingGaps`、`DiskTooSlow`(再生中の読み込み)、`BounceFailed`(`msg.bounceFailedWrite` の「音にできませんでした。作業の場所に書けません。」)、`ClipWorkFailed`(クリップについて)がある。どれも「ファイルが作れないので、声が録れていない」という内容ではなく、それぞれ別の原因を書いている。そのため使うと誤解を招く。`ui/src/text/ja.ts` にも、ほかに合う文言はない(保存のダイアログの「ここには保存できませんでした(書きこむ権利がないか、空きが足りません)…」は、曲の保存先についてのもの)。

`needs-lead` とする理由: 修正には、新しい文言(仕様 6 にはない)と、新しい `StatusMessage` の種類(プロトコルの追加。版を決める必要がある)が要る。選択肢:
1. 新しい種類(例: `VocalsNotStarted`)を作り、文面はオーナーさんに決めてもらい、`Input::VocalsNotStarted` の処理で設定する。録音が声のトラックだけなら、そこで止める(何も録れないため)。費用: プロトコル、`ja.ts` / `en.ts`、状態行の表、架空の場面。推奨。文面の案は `msg.bounceFailedWrite` の言い方にそろえる: 「声を録れません。作業の場所に書けません。」(キーの演奏も一緒に録音している場合は、キーは続く)。
2. 新しい文言は作らず、すぐに録音を止め、テイクを作らず、何も言わない。それでも知らせがないので、推奨しない。
3. そのままにし、`docs/progress.md` に書き出す。

深刻度 `medium` はそのまま。歌った一回分が、何も残らずに消えうるため。作業の場所に書けないことが条件だが、影響は大きい。

## Fix

プロトコル 38(リードの決定): `StatusMessage::VocalsNotRecorded { keys }`(`crates/meloet-protocol/src/lib.rs`)。`crates/meloet-core/src/play/vocal.rs` の `vocals_not_started` は、録音が鍵の演奏とともに続くかどうかを返す。`crates/meloet-core/src/state.rs` の `Input::VocalsNotStarted` の処理は、知らせを設定し、声のトラックだけの録音なら止める。`Model::status` は、この知らせを録音自身の行より前に出すので、すぐに見える。文言は `msg.vocalsNotRecorded` と `msg.vocalsNotRecordedKeys`(`ui/src/text/ja.ts`、`en.ts`。`builder:2026-11-25`、仕様 7 の A-24「録音やファイルを書けないときの文言」)。表示は `ui/src/components/StatusLine.svelte` が行う。テスト: `state::vocal_tests::a_recording_of_vocals_alone_whose_files_cannot_be_made_stops_and_says_so`、`state::vocal_tests::a_recording_stopped_before_it_rolls_leaves_no_file` の「Said at once, over the recording that goes on」の手順、`tests38::the_messages`(プロトコル)、「知らせ」の行(`ui/tests/connect-components.test.ts`)。
