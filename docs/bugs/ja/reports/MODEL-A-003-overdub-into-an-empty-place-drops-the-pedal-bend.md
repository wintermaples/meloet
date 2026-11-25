---
id: MODEL-A-003
title: 空いている場所への重ね録りで、最初のノートの前から押されていたペダル・ベンド・モジュレーションが消える
area: MODEL-A
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# MODEL-A-003: 空いている場所への重ね録りで、最初のノートの前から押されていたペダル・ベンド・モジュレーションが消える

## Where

crates/meloet-model/src/record/overdub.rs:58-65 (ここにクリップはなく、テイクは `in_the_gap` を通る)
crates/meloet-model/src/record/overdub.rs:252-287 (`in_the_gap`: `cut = low - take.origin`、そして操作の点に `.filter(|p| p.tick >= cut && …)`)

## What happens

「重ねる」を入れた状態で、トラックのどのクリップも録音したノートの開始を含まないとき、`overdub_recording` はテイクを、ノートの周りの空いている場所に切り取る。`low` から(最初のノートの前、または取り始めの前に終わる最後のクリップの終わり、またはテイクの開始から)次のクリップまで。`cut` より前にあるコントロールの点はすべて捨てられる。それには、録音の開始(またはループの一周の開始)のときに押されたままだったペダル、ベンド、モジュレーションのために、録音機がテイクの tick 0 に書く点も含まれる (`HeldControls`、`Lap::head`)。また、開始の後で `low` より前に押されたペダルも含まれる。新しいクリップは、最初のノートの下でペダルが踏まれていない(またはベンドもモジュレーションもない)ことになる。使う人は演奏中にそれを押し、聞いていたのに。再生すると、次のペダルの点まで、ノートは乾いて鳴る。点は曲から消え、切り取られるだけではない。

## What should happen

録音は、弾いたものを保つ。クリップがコントロールの開始を含まないところでは、モデル自身の規則は、クリップの開始より前にある最後のペダル、ベンド、モジュレーションを開始まで運ぶことである。`track_events` は、切り詰めた頭についてこれをする(「開始より前の最後のペダル、ベンド、モジュレーションは、休止でなければ開始に置かれる」、crates/meloet-model/src/playback.rs:30-34、architecture 13「クリップの鳴らし方」)。録音機は、押されたままのペダルを各一周の頭に書く(architecture 13「ループの録音」)。「重ねる」を入れなければ、`add_recording` はテイクのコントロールの点をすべて残す。

## How to trigger

コードから読んだもので、実行はしていない。
1. 鍵盤のトラックに、小節 1 にクリップ A がある(どんなノートでもよい)。「重ねる」を入れる。
2. 再生の線を小節 1 に置き、サステインペダルを踏んだまま R を押す。
3. A の終わりの後、小節 2 からだけノートを弾く。ペダルは踏んだままにして、止める。
4. 新しいクリップは A の終わりから始まる。そのテイクには `CC 64 = 127` の点がないので、ノートはペダルなしで鳴る。A と空いている小節の上で、各一周が頭に踏まれたペダルで始まるループの録音でも、同じことが起きる。

## Cause

`in_the_gap` はテイクの tick 0 を `cut` ティック分だけ `low` へ動かし、`[cut, cut + length)` の中のコントロールの点だけを残す。`cut` で有効になっている値は見ないので、頭の点 (tick 0) と、`cut` より前のペダル、ベンド、モジュレーションの変化はすべて消える。

## Suggested fix

`in_the_gap` で、絞り込みの前に、テイクのコントロールのうち `cut` 以前にある最後のサステイン (CC 64)、モジュレーション (CC 1)、ピッチベンドの値を見つける。休止でないものがあれば、(同じコントローラーの点が `cut` にすでにない限り) 切り取ったテイクの tick 0 に置く。これは `track_events` と録音機の一周の頭の規則と同じである。プロトコルやファイルの形の変更はない。テストは `with_no_clip_there_a_new_clip_fills_only_the_empty_place` の隣に、`controls = [pedal(0, 127)]` で書く。

## Verification

私が実行したテストで確認した (作業用の scratch worktree、`cargo test -p meloet-model --lib verify_model_a_003`): 小節 1-2 にクリップがあり、続けて tick 0 から、tick 0 に `CC 64 = 127` があり小節 3 に一つのノートがあるテイクを `overdub_recording` する。新しいクリップ (小節 3-4) にはコントロールの点がなく、古いクリップも何も受け取らない。ペダルは曲から消えている。重大度 `medium` はそのままにする(録音の始めからペダルを踏むのは普通の弾き方であり、回避策は点を手で書くことだけ)。

訂正: なし。

Fix plan: `crates/meloet-model/src/record/overdub.rs` の `in_the_gap`: 絞り込みの前に、tick `< cut` にある最後の CC 64、CC 1、ピッチベンドの値を取る(テイクのコントロールは tick で並んでいる)。休止でなく、`cut` にちょうど同じコントローラーの点がないものは、切り取ったテイクの tick 0 に置く。`playback.rs` の `track_events` にすでにある、切り詰めた頭の規則を再利用する(共有できるなら。二つ目の規則は書かない)。テストは `record/overdub/tests.rs` の `with_no_clip_there_a_new_clip_fills_only_the_empty_place` の隣に置く。

## Fix

`crates/meloet-model/src/record/overdub.rs` の `in_the_gap`: 切り目より前の点は、録音機自身の頭の規則 (`record.rs` の `set_head`) を通す。休止でないペダル、ベンド、モジュレーションは、切り取ったテイクの tick 0 に置く。ただし、ちょうど切り目に点があるコントローラーは除く。`docs/architecture.md` 24-7 にそう書いてある。

テスト: `record::overdub::tests::with_no_clip_there_the_new_clip_starts_with_what_was_held` (meloet-model)。
