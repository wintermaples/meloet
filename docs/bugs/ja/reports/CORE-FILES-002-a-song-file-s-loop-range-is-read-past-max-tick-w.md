---
id: CORE-FILES-002
title: 曲のファイルのループの範囲が MAX_TICK を超えて読まれ、set_loop がそれを拒む
area: CORE-FILES
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-FILES-002: 曲のファイルのループの範囲が MAX_TICK を超えて読まれ、set_loop がそれを拒む

## Where

crates/meloet-core/src/songfile.rs:1823-1826 (`loop_range: Some(TickRange::new(r.start, r.end)?)`。`MAX_TICK` の確認がない)
crates/meloet-model/src/timeline.rs:306-308 (`set_loop` は `range.end() > MAX_TICK` を拒む)
crates/meloet-core/src/play.rs:437 (`ceil`: `r.end() as i64`)

## What happens

`"loop": {"start": 0, "end": 18446744073709551615}` の曲ファイルが開いてしまう。するとループの終わりは、モデルが許すどの tick よりも大きくなる。コアが i64 として扱う場所では、値が折り返される。`play.rs:437` では、2^63 以上の終わりに対して天井が -1 になる。その後、ループがオンの状態で録音すると、`press` (`play.rs:470`) はすべての生のノートを tick 0 に置き (`.min(ceil - 1).max(0)`)、`release` (`play.rs:462`) はその長さを 1 にする。`play.rs:2162` は生の u64 を画面へ送るが、JS の数はそれを正確に保てない。エンジンも同じ終わりを伝えられ (`play.rs:1031-1037`、`EngineCommand::SetLoop`)、テンポの地図によってフレームの数に変換する。

## What should happen

ループの範囲は、`set_loop` が守っている規則を守る。その終わりは最大でも `MAX_TICK` である (meloet-model/src/timeline.rs:306)。モデルの規則を破る曲は読まない (docs/architecture.md 14)。デコーダーは、クリップの終わり、マーカー、テンポの点、オートメーションの点を `MAX_TICK` までに抑えている。ループの範囲は、その中で唯一、tick の欄を通してしまうものである。

## How to trigger

実行して確かめたのではなく、コードを読んで確かめた。保存済みの曲の `"loop"` を `{"start": 0, "end": 18446744073709551615}` に書き換えて開き、ループをオンにし、キーをいくつか録音して弾く。録音中に表示される生のノートは、tick 0 に長さ 1 で置かれる。

## Cause

`to_song` はループを `TickRange::new(r.start, r.end)` で作る。これは `end > start` しか確かめない。`timeline::set_loop` がする `MAX_TICK` の確認が、ここにはない。

## Suggested fix

`to_song` で、`r.end <= MAX_TICK` も求める (`TickRange::new(r.start, r.end).filter(|_| r.end <= MAX_TICK)?`。songfile.rs:1665 のクリップと同じ)。プロトコルやファイルの形は変えない。meloet はそのようなループを書かない。

## Verification

テストを実行して確かめた (作業用の worktree。`cargo test -p meloet-core --features dev-hooks --lib verify_core_files_002`): ループのある曲を符号化し、JSON の中でループの `"end"` を `18446744073709551615` に変えた。`songfile::decode` は `Ok` を返し、`loop_range: Some(TickRange { start: 0, end: 18446744073709551615 })` になった。`timeline::set_loop` は `MAX_TICK` を超える終わりを拒み、`play.rs:437` は終わりを `i64` に変換する。その後、録音とエンジンがそれをどう扱うかは、読んだだけで、実行はしていない。重大度 `low` は変えない (手で変えられたファイルのため)。

訂正: なし。

修正計画: `crates/meloet-core/src/songfile.rs` の `to_song`: `Some(r) => Some(TickRange::new(r.start, r.end).filter(|_| r.end <= MAX_TICK)?)`。テストは `songfile/tests.rs` に置く。

## Fix

`crates/meloet-core/src/songfile.rs` の `to_song`: 終わりが `MAX_TICK` を超えるループは読まれない。`timeline::set_loop` がそれを拒むのと同じである。`docs/architecture.md` 14 はこのことを書いている。

テスト: `songfile::tests::ticks_past_max_tick_do_not_read` (meloet-core。終わりが `u64::MAX` と `MAX_TICK + 1` のループは壊れていると扱われ、`MAX_TICK` は読まれる)。
