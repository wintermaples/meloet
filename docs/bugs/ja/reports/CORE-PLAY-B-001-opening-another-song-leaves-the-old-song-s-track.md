---
id: CORE-PLAY-B-001
title: 別の曲を開いても、前の曲のトラックのレーンがエンジンで鳴り続ける
area: CORE-PLAY-B
kind: bug
severity: high
status: fixed
found: 2026-11-25
---

# CORE-PLAY-B-001: 別の曲を開いても、前の曲のトラックのレーンがエンジンで鳴り続ける

## Where

crates/meloet-core/src/play/autosync.rs:147-152 (`all` が `sent` を空にする)
crates/meloet-core/src/play/autosync.rs:183-186 (レーンがなく `sent` にも項目がない座席は飛ばされる)
crates/meloet-core/src/play/seats.rs:195-210 (`reseat` は新しい曲が使う座席を、`ClearSeat` なしで残す)
crates/meloet-core/src/play.rs:2076-2106 (`replace_song` → `reseat` → `stream_opened` → `send_automation(true)`)

## What happens

オートメーションのレーンを持つトラックがある曲を、出力が開いたままの状態で別の曲に入れ替えると（「新しい曲」、曲を開く、最近の曲を開く）、新しい曲のトラックのうち、前の曲も使っていた座席に置かれていて、自分のレーンを持たないものが、前の曲のレーンで動き続けます。たとえば、前の曲のトラック 1 には −∞ まで下がる音量のレーンがあり、新しい曲のトラック 1 にはレーンがないとします。新しい曲を開くと、新しい曲のトラック 1 は（止まっていて再生線があるときも）前の曲の音量の線に従い、曲の中のフェーダーは効かなくなります。パン、センド、ミュート（ミュートのレーンがあると無音になります）、内蔵音源のつまみ、同じ場所にある、または音源として使われているエフェクトやプラグインのパラメータでも同じことが起きます（前の曲のレーンのパラメータ番号が、新しいプラグインに送られます）。そのトラックのレーンが変わる（トラックにレーンが追加される）まで続きます。

## What should happen

エンジンは、開いている曲のレーンだけを再生し、ほかは再生しません（docs/architecture.md 21-7: 「コアは、トラックごとの止まっていないレーンの点を 1 つの箱にまとめる」。21-5: 点のないレーン、またはレーンがないときは、手で設定した値のままになる）。同じ `reseat` から呼ばれる兄弟の `send_audio(false)`（`play/audiosync.rs:119-130`）は正しく動いています。`fresh == false` のとき、使われているすべての座席に、空のリストを付けた `EngineOp::Clips` を送るので、前の曲のオーディオクリップは消えます。マスターのレーンも、「なし」の場合を含めて、いつも通知されます（`autosync.rs:153-161`、architecture 25-32）。

これは docs/progress.md の 377 行目（「ラウンド 12」の節）の「ストリームが動いていない間に、トラックのレーンを削除または休ませると、エンジンに残る」という既知の穴とは違います。ここではストリームは動き続けていて、レーンを削除したり休ませたりしたのは使う人ではありません。

## How to trigger

コードを読んで確認したもので、実行はしていません。
1. 出力を開いた状態で、トラック 1 に点のある音量のレーン（または 1.0 の点のあるミュートのレーン）を持つ曲を作る。
2. 「新しい曲」（「保存しない」と答える）、または、トラック 1 にレーンがない別の曲を開く。
3. 再生する、または再生線を動かす。新しい曲のトラック 1 は前のレーンに従い、前のミュートのレーンがあると無音になる。

## Cause

`replace_song` は `reseat` を呼び、それは新しい曲のトラックに座席 0..n を割り当てます。両方の曲が使う座席は空にされません（`clear_seat` は、新しい曲が使わない古い座席にしか働かず、`seats.rs:204-208`）。そのため、エンジンのトラックは `AutoSet` を持ったままになります（`EngineCommand::ClearTrack` だけが、`SetAutomation` 以外でこれを捨てます）。次に `stream_opened` が `send_automation(true)` を呼びます。これはまず `self.autosync.sent` を空にしてから、座席ごとに、`before.map_or(heads.is_empty(), …)` が真のとき（つまり、新しいトラックに動いているレーンがないとき）その座席を飛ばします。`sent` が空なので `before` は `None` になり、動いているレーンのない座席はすべて飛ばされ、`EngineOp::Automation { seat, lanes: [] }` は送られません。`gone` の処理も助けになりません。そこが歩くのは、直前に空にされた `sent` の対応表だからです。

## Suggested fix

`send_automation` で、`all` が真のとき、`now` にあって heads が空の座席ごとに `EngineOp::Automation { seat, lanes: Vec::new() }` を送る（または、`all` が真のときは heads が空の座席も飛ばさない）。マスターのレーンが「なし」の場合も含めて通知されるのと同じです。これで docs/progress.md 377 行目の既知の穴（新しいストリームへの対応）も塞がります。プロトコルやファイルの形式は変えません。

## Verification

自分で実行したテストで確認しました（作業用の worktree で `cargo test -p meloet-core --features dev-hooks --lib verify_core_play_b_001`）。ストリームを開いた状態（`ready()`）で、トラック 1 に点が 2 つある音量のレーンを作り、`Play::replace_song(Song::new_default(), ..)` を呼びます。古い曲と新しい曲の両方のトラック 1 は座席 0 にあり、置き換えの効果の中に `EngineOp::Automation` も `EngineOp::ClearSeat` も一つもありません。エンジン側（コードを読んで確認）では、トラックの `AutoSet` は `EngineCommand::SetAutomation`（`engine.rs:1951-1957`）でだけ置き換えられ、`ClearTrack`（`engine.rs:2093-2112`）でだけ捨てられます。別の楽器を座席に載せても、それには触れません。そのため、前の曲のレーンは、その座席にある新しい曲のトラックを動かし続けます。エンジンを動かして音を聞いてはいません。重大度 `high` はそのままです（レーンのない曲でも、音が違ったり無音になったりし、フェーダーが効かなくなります）。

訂正: なし。これは `docs/progress.md` の既知の行（「ストリームが動いていない間に、トラックのレーンを削除または休ませると、エンジンに残る」）より広い問題です。ここではストリームが動き続けています。

修正の計画: `crates/meloet-core/src/play/autosync.rs` の `send_automation`。`all` のとき、heads が空の座席を飛ばさず、`EngineOp::Automation { seat, lanes: Vec::new() }` を送る（マスターのレーンが「なし」を通知されるのと同じ）。新しい曲が使わない座席は、`reseat` がすでに空にしています。テストは `crates/meloet-core/src/play/automation/tests.rs` に置く。置き換えの後、使われているすべての座席が `Automation` の操作を受け取り、レーンのないトラックの操作は空であること。修正がテストで正しいと確認できたら、新しいストリームについて `docs/progress.md` の既知の行も削除する。

## Fix

`crates/meloet-core/src/play/autosync.rs` の `send_automation`: `all`（新しいストリーム、別の曲）のとき、再生中のレーンがない座席を飛ばさず、`EngineOp::Automation { seat, lanes: [] }` を送る。マスターのレーンが「なし」を通知されるのと同じです。これで `docs/progress.md` の既知の穴（ストリームが動いていない間に、トラックのレーンを削除または休ませると、エンジンに残った）も塞がり、新しいストリームは座席ごとのレーンを全部通知されます。`docs/architecture.md` 25-32 にもそう書きました。

テスト（`play/autosync.rs` 内）: `another_song_leaves_no_lane_of_the_one_before_on_its_seats`（トラック 1 に音量のレーンを置いた後、「新しい曲」→「保存しない」。新しい曲のすべての座席にレーンなしで通知される）と、`a_new_stream_hears_of_a_lane_deleted_while_none_ran`。正しい動作に合わせて変更: `a_song_without_lanes_tells_the_engine_nothing` は `a_song_without_lanes_tells_a_new_stream_so_and_then_nothing` になり（新しいストリームには座席ごとに 1 回「レーンなし」と通知され、その後は何も通知されない）、`play/tests.rs` の `the_stream_opening_gets_everything` は `Automation { seat: 0, lanes: [] }` も期待するようになりました。
