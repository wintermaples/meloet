---
id: CORE-STATE-001
title: 一回の起動で二回目以降の録音は、待っている変更をなおすためのファイルへ先に書かなくなった
area: CORE-STATE
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-STATE-001: 一回の起動で二回目以降の録音は、待っている変更をなおすためのファイルへ先に書かなくなった

## Where

crates/meloet-core/src/state/recovery.rs:208-217 (`recovery_tick`: `let started = busy && !std::mem::replace(&mut self.recovery.was_busy, busy);`)
crates/meloet-core/src/state/recovery.rs:54-55 (`was_busy`。一度 true になると、false に戻されない)

## What happens

一回の起動で最初の録音(またはカウントイン)だけが、まだ 2 秒を待っている変更を、すぐになおすためのファイルへ書く。二回目以降の録音では、録音の始まりの 2 秒前に行った変更は、録音の始まりでは書かれない。録音が終わるまで待つ。その録音の途中で meloet が終わると(クラッシュ、SIGKILL、停電)、次の起動では、最後の変更の前の曲が戻る。そして `work/rec.json` から残した声の録音が、その古い曲に載せられる(テンポマップが違う、もうないトラックがある、など)。

## What should happen

同じ関数のコメント: 「録音は、まだ待っている変更の上に始まる。そのときに曲を今書く。そこから録音される声はこの曲の上に置かれ、録音の途中で終わったあとの起動は、変更の前の曲ではなく、これを見つけなければならない。」これは最初の録音だけでなく、すべての録音に当てはまる。docs/architecture.md 15(変更の 2 秒後に書く。「録音中とカウントイン中は書かない。録音したものは曲に入ったあとに書く」)は、録音の始まりでこの書き込みがあることを前提にしている。

## How to trigger

コードを読んで確かめたもので、実行はしていない。

1. meloet を起動し、カウントインを 0 にする。短いテイクを録音して止める(どの録音でもよい。これで `was_busy` が永久に true になる)。
2. 何かを変える(例: `SetTempo`、または歌のトラックを追加する)。2 秒以内に録音を押す。
3. その録音のエフェクトには `Effect::WriteRecovery` がない(最初の録音と同じ手順なら、ある。`nothing_is_written_while_recording_and_the_take_is_written_after` のテストを見る)。
4. 録音の途中で meloet を殺す。なおすためのファイルには、手順 2 の前の曲が残ったままである。

既存のテスト `a_recording_over_a_song_that_is_written_already_writes_nothing_more` も二回目の録音をするが、その変更はすでに書かれていたので、この不具合があっても通る。

## Cause

`busy && !std::mem::replace(&mut self.recovery.was_busy, busy)` では、`&&` は `busy` が false のとき右側を実行しない。そのため `was_busy` は busy のときにしか書かれず、最初の録音が終わったあとも `true` のままになる。その後の録音の始まりでは、`replace` が `true` を返すので `started` は `false` になり、`if busy` の中の早い `return` が書き込みを飛ばす。

## Suggested fix

`was_busy` を毎回更新する。例: `let was_busy = std::mem::replace(&mut self.recovery.was_busy, busy); let started = busy && !was_busy;`。テストを足す。一回録音し、そのあと変更して 2 秒以内にもう一度録音すると、二回目の Record から `WriteRecovery` が出ること。プロトコルや、ファイルの形は変えない。

## Verification

読んで確かめた(テストは実行していない。その一行だけで決まる)。`recovery_tick`(`state/recovery.rs:209`): `let started = busy && !std::mem::replace(&mut self.recovery.was_busy, busy);`。`&&` は `busy` が false のとき右側を実行しないので、`was_busy` は busy のときにだけ書かれ、最初の録音の一回のあとも戻されない。それ以降の録音の始まりでは `replace` が `true` を返し、`started` は false になる。そして、`if busy { … return; }` の中は何も書かない。その中のコメントは、待っている変更の上に始まるすべての録音のために書き込むことを意図している。深刻度 `medium` はそのまま。録音の途中で終わると、最後の変更の前の曲が戻り、残った声がその曲に置かれるため。

訂正: なし。

修正計画: `crates/meloet-core/src/state/recovery.rs` の `recovery_tick`: `let was_busy = std::mem::replace(&mut self.recovery.was_busy, busy); let started = busy && !was_busy;`。テスト: `state/recovery/tests.rs` の `nothing_is_written_while_recording_and_the_take_is_written_after` の隣に、一回録音して止め、そのあと変更して 2 秒以内に録音すると `Effect::WriteRecovery` が出ることを書く。

## Fix

`crates/meloet-core/src/state/recovery.rs` の `recovery_tick`: `was_busy` は、すべての入力のあとに設定される(`let was_busy = std::mem::replace(..); let started = busy && !was_busy;`)。これで、待っている変更の上に始まるすべての録音かカウントインが、一回の起動の最初のものだけでなく、すぐに変更を書く。

テスト: `state::recovery::tests::every_recording_over_a_waiting_change_writes_it_first`(`state/recovery/tests.rs`)。
