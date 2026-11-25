---
id: CORE-FILES-001
title: 曲のファイルのノートとコントロールの点は、どんなティックでも読まれ、Note::end があふれる
area: CORE-FILES
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-FILES-001: 曲のファイルのノートとコントロールの点は、どんなティックでも読まれ、Note::end があふれる

## Where

crates/meloet-core/src/songfile.rs:1909 (`to_take`: ノートは長さ 1 以上、鍵と強さだけを確かめる。`start` と `length` はどんな u64 でもよい)
crates/meloet-core/src/songfile.rs:1922-1931 (コントロールの点の `tick` はどんな u64 でもよい)
crates/meloet-core/src/songfile.rs:1683 (`MAX_TICK` に従うのはテイクの `length` だけで、中のものは従わない)
crates/meloet-model/src/song.rs:58-60 (`Note::end` は `self.start + self.length`)
crates/meloet-model/src/playback.rs:46,60 (`at(tick) = zero + tick as i64`、`at(n.end())`)

## What happens

テイクが `[1, 18446744073709551615, 60, 100]` のようなノート、または `[0, 9223372036854775808, 60, 100]` を持つ曲のファイルは、何の断りもなく開けます。曲のイベントがエンジン用に作られるとき (`playback::track_events`、開くたびに実行される)、前者では `n.end()` が u64 からあふれ、後者では `zero + tick as i64` が i64 からあふれます。デバッグビルド (オーナーが VS Code から動かすもの) はコアのスレッドでパニックします。リリースビルドでは桁が巻き戻り、ノートオフは負のティックに入り、`.min(end) as u64` で巨大なティックになるので、そのノートは終わらない (または始まる前に終わる) ことになります。`i64::MAX` を超えるティックのコントロールの点も、同じく `at(c.tick)` で巻き戻り、クリップの前に入ります。

## What should happen

モデルが守るべき規則を破る曲は読まない (docs/architecture.md 14、「Does not read: ... a break of a model rule」。依頼の趣旨: 壊れた、または悪意のある JSON でパニックしてはならない)。モデル自身の編集は、そのような値のノートを作らない。`meloet-model/src/notes.rs:190` は `MAX_TICK` 以上から始まるノートを拒み、`notes.rs:211` は長さを `MAX_TICK - start` に抑える。`songfile.rs` は、すでにクリップの終わり、クリップの開始位置、マーカー、テンポの点、オートメーションの点について `MAX_TICK` を超えるものを拒んでいる。ノートとコントロールの点が抜けている。

## How to trigger

コードを読んで確かめた内容で、実行はしていません。キーのクリップを一つ持つ曲を保存し、`.meloet` ファイルを手で編集して、そのクリップが示すテイクの中のノートを一つ `[1, 18446744073709551615, 60, 100]` にする。デバッグビルドで開くと、コアのスレッドが「attempt to add with overflow」で `Note::end` でパニックする。`[0, 9223372036854775808, 60, 100]` では、パニックは `playback.rs:46` (`zero + tick as i64`) にある。リリースビルドでは、二つ目のノートは再生が止まるまで鳴り続ける。

## Cause

`to_take` (songfile.rs:1896-1944) は、ノートとコントロールの点の順番、鍵、強さ、長さ 1 以上は確かめるが、`start`、`start + length`、コントロールの点の `tick` が `MAX_TICK` (`i64::MAX / 2`) の中に収まるかは確かめない。それ以降のすべての使い方はそれを前提にしている。`Note::end` は確かめずに足し、`playback.rs` はティックを i64 にキャストする。

## Suggested fix

`to_take` で、ノートが `start >= MAX_TICK` であるか、`start.checked_add(length)` が `None` か `MAX_TICK` 超えのとき、そしてコントロールの点の `tick > MAX_TICK` のとき、テイクを拒む (`None` を返し、ほかの壊れた規則と同じく曲は読まれない)。プロトコルもファイルの形も変えない。meloet が書いた曲にそのような値は決して入らないので、本物の曲が読めなくなることはない。

## Verification

テストを実行して確かめた (作業用の scratch worktree、`cargo test -p meloet-core --features dev-hooks --lib verify_core_files_001`): クリップ一つの曲を作って符号化し、ノート `[37,200,60,90]` を JSON の中で `[37,18446744073709551615,60,90]` に変えた。`songfile::decode` は `Ok` を返し、そのあと `meloet_model::playback::track_events` がパニックする (`crates/meloet-model/src/song.rs:59`、「attempt to add with overflow」、デバッグビルド)。リリースビルドは報告のとおり巻き戻る (読んだだけで、実行はしていない)。重大度 `low` のまま: そのようなノートは、手で変えられたか壊れたファイルにしかなく、meloet は決して書かない。

訂正: なし。

修正案: `crates/meloet-core/src/songfile.rs` の `to_take`: ノートは、`start < MAX_TICK` かつ `start.checked_add(length)` が `Some(end)` で `end <= MAX_TICK` のときだけ取る。コントロールの点は、`tick <= MAX_TICK` のときだけ取る。それ以外はテイクが `None` になり、ほかの壊れた規則と同じく曲は読まれない。テストは `songfile/tests.rs` に置く (報告の二つのノート、`MAX_TICK` を超えて終わるノート、それを超えるコントロールの点は、どれも `Err(Broken)` になる)。

## Fix

`crates/meloet-core/src/songfile.rs` の `to_take`: 中のノートの `start + length` があふれる、または `MAX_TICK` を超える、あるいはコントロールの点が `MAX_TICK` を超えるテイクは、読まれない (`ReadError::Broken`。ほかの壊れた規則と同じ)。`docs/architecture.md` 14 にそう書かれている。

テスト: `songfile::tests::ticks_past_max_tick_do_not_read` (meloet-core。報告の両方のノート、`MAX_TICK` を超えて終わるノート、それを超えるコントロールの点は壊れている。同じ値が `MAX_TICK` にあるときは読まれ、`track_events` はパニックしない)。
