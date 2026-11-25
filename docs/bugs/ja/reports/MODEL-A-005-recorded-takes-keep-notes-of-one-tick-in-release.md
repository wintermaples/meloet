---
id: MODEL-A-005
title: 録音したテイクは、同じティックのノートを離した順に保つため、何も変えない編集でも一つの手になる
area: MODEL-A
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# MODEL-A-005: 録音したテイクは、同じティックのノートを離した順に保つため、何も変えない編集でも一つの手になる

## Where

crates/meloet-model/src/record.rs:301 (`Recorder::take`: `notes.sort_by_key(|n| n.0)`。開始だけで並べる。`lap.notes` は鍵を離した順に入る。record.rs:359)
crates/meloet-model/src/notes.rs:873 と 876-885 (`put_take_reaching` は新しいノートを `(start, key)` で並べ、編集が `Unchanged` を返すのは、新しいノートが古いノートと、古い順のまま等しいときだけ)

## What happens

録音したテイクは、同じティックで始まるノートを、鍵を離した順に保つ(開始だけで並べる安定ソート)。一つのコードのキーのイベントが PC にほぼ同時に届くと(120.00 では一ティックは 0.26 ms。一つの USB MIDI のパケットのイベントは数マイクロ秒おきに読まれる)、すべてに一つのティックが付くことがある。アルペジオと「さっきの演奏を残す」も同じ `Recorder` を通る。すると C-E-G で G を最初に離した場合、G, E, C の順で保存される。

どのノートの編集も、新しいノートを作り、それを `(start, key)` (C, E, G) で並べてから、古いノートをテイクの順 (G, E, C) と比べる。両者は違うので、どのノートも変えない編集は `Unchanged` にならない。新しいテイクと、曲に何もしない取り消しの手ができる。例: 「ベロシティをまとめて変える」の「割合」100 %、ノートがすでに次の開始まで届いているときの「次の音までのばす」、ノートがすでに持つベロシティを打ち込む、127 でのベロシティの一段上げ。(量子化したテイクはすでに並んでいるので、影響があるのは、まだ編集していない録音のテイクだけ。) 使う人は履歴の行 (「ベロシティを割合で変える」 …) を見て、取り消しをもう一回押す必要がある。

## What should happen

architecture 13: 「何も変えない操作は手を作らない」。architecture 25-10: ベロシティと長さの道具について「変化なし → 手なし」。architecture 16: 「テイクのノートは『開始 → 鍵 → 先に来たもの』の順に並ぶ(安定ソート)」。ほかの鍵を作る処理は、どれもこの順に従う(`write_in_new_clip` notes.rs:354、`join_clips` arrange.rs:592、`overdub_recording` record/overdub.rs:127、`put_take_reaching`)。

## How to trigger

コードから読んだもので、実行はしていない。
1. 同じティックになるノートのコードを録音し、一番上の鍵から先に離す(または、同じティックに二つのノートがあり、高い鍵が先の録音したテイク)。
2. そのコードを選び、「ベロシティをまとめて変える」 → 「割合」100 → 適用。
3. 履歴に「ベロシティを割合で変える」という手が出る。ノートは変わっていない。

## Cause

`Recorder::take` は開始だけで並べるので、テイクは編集が前提にしている `(start, key)` の順を壊す。`put_take_reaching` は、並べ替えた新しい一覧を、並べていない古い一覧と比べる。

## Suggested fix

`Recorder::take` で `(start, key)` で並べる (`notes.sort_by_key(|n| (n.0, n.2))`、安定ソート)。ほかのテイクを作る処理と同じにする。必要なら `live_notes` も同じように並べ、ライブの表示と合わせる。プロトコルやファイルの形の変更はない。保存済みのテイクはその順のまま残り、今と同じに動く。

## Verification

私が実行したテストで確認した (作業用の scratch worktree、`cargo test -p meloet-model --lib verify_model_a_005`): 同じティックで三つの鍵を押し、一番上から離すと、鍵が `[67, 64, 60]` のテイクになる。`put_take_reaching` (`notes.rs:873-885`) は新しいノートを `(start, key)` で並べ、古いノートはテイクの順のまま比べる。そのため、そのようなテイクでは、何も変えない編集は `Unchanged` にならない (コードを読んだ確認で、最後まで実行はしていない)。重大度 `low` はそのままにする: 同じティックに二つの鍵が必要で、代償は空の履歴の行が一つである。

訂正: なし。

Fix plan: `crates/meloet-model/src/record.rs` の `Recorder::take`: `notes.sort_by_key(|n| (n.0, n.2))`。テストは `record/tests.rs` に (同じティックのコードの順)。`live_notes` は、画面の index がライブのテイクから最終的なテイクに合う必要がない限り、そのままにする。修正のときに確かめる。

## Fix

`crates/meloet-model/src/record.rs` の `Recorder::take`: ノートを `(start, key)` で並べる (安定ソート)。`live_notes` はそのまま (描くだけで、intent はそこを指さない)。

テスト: `record::tests::notes_of_one_tick_are_in_the_order_of_key` (meloet-model。一番上から先に離したコードは鍵 `[60, 64, 67]` になり、`change_velocity` の 0 は `Unchanged`)。
