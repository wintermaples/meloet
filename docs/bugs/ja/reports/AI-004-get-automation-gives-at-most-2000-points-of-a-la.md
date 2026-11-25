---
id: AI-004
title: get_automation はレーンの点を最大 2000 個しか返さず、ページ送りがないので、後ろの点は見えず、名指しもできない
area: AI
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# AI-004: get_automation はレーンの点を最大 2000 個しか返さず、ページ送りがないので、後ろの点は見えず、名指しもできない

## Where

crates/meloet-ai/src/tools/automation.rs:217 (`lane.points.iter().enumerate().take(MAX_NOTES)`)
crates/meloet-ai/src/tools/automation.rs:108-111 (`TrackArgs`: `offset` / `limit` がない)

## What happens

`get_automation` は、レーンごとに最初の 2000 個の点だけを、横に `total` を付けて返します。残りを取るための `offset` も `limit` もありません。`move_automation` と `delete_automation` は、`get_automation` が返す番号で点を指定するので、番号が 2000 以降の点は、見ることも、その場所を知って名指しすることもできません。レーンの点の数には上限がなく（`meloet_model::automation::put_points` には上限がない）、再生中に（「● 書く」で）書かれたレーンや、`write_automation` を続けて呼んで書かれたレーンは、2000 を超えることがあります。

## What should happen

すべての点を読めるべきです。「長い一覧はページで返す: `limit`、`offset`、そして `total`、`count`、`next_offset`」（docs/development/ai-tools.md の「すべての道具の答え方」。道具の INSTRUCTIONS も同じことを言っています）。兄弟の `get_notes` は、同じ理由で `first` と `max` を持っています。

## How to trigger

コードを読んで確認したもので、実行はしていません。1 つのレーンに 2001 個以上の点を書く（違う tick で `write_automation` を 2000 個ずつ 2 回呼ぶ）。その後 `get_automation` を呼ぶと、`total` は 2000 を超えますが、最後の点は返ってこず、それを頼む方法もありません。

## Cause

2000 の上限（`MAX_NOTES`）は、ページ送りの引数なしで一覧に付けられていました。

## Suggested fix

`get_automation` に、任意の `offset`（または `first`）と `limit`（既定値も最大も 2000）を加える。レーンごと、または `target` で選んだ 1 つのレーンに適用する。残りの点があるとき `next_offset` を返す。番号は、レーンの中の番号のままにする。プロトコルは変えない。docs/development/ai-tools.md に 2 つの引数を加える。

## Verification

コードを読んで確認しました。`get_automation` は `track`（`TrackArgs`）だけを受け取り、`lane.points…take(MAX_NOTES)`（2000）と `total` を返します。レーンの最初の 2000 個の後の点を読む方法はありませんが、`move_automation` と `delete_automation` は、このツールが返す番号で点を名指しします。これを欠陥として扱い、希望とはしなかったのは、兄弟の道具に欠けている半分があるからです。`get_notes` は同じ上限に対して `first` と `max` を取り（`tools.rs:705-710`）、道具の指示と `docs/development/ai-tools.md` には「長い一覧はページで返す」とあります。レーンの点には上限がありません。重大度 `low` はそのままです（点が 2000 を超えるレーンは珍しい）。

訂正: なし。

修正の計画: `crates/meloet-ai/src/tools/automation.rs`: `get_automation` に専用の引数を持たせる（`track`、`get_notes` と同じ名前の任意の `first` と `max`。既定値も最大も 2000）。これをすべてのレーンの点に適用する。各レーンは `total` を保ち、`next_first` を加える（残りがなければ null）。番号はレーンの中の番号のままにする。`docs/development/ai-tools.md` に 2 つの引数を書く。テスト: `crates/meloet-ai/tests/` に、2001 個の点のあるレーン（2 ページ目に番号 2000 がある）。

## Fix

`crates/meloet-ai/src/tools/automation.rs`: `get_automation` は、任意の `first` と `max` を取る（`get_notes` と同じ名前。既定値も最大も 2000）。これをすべてのレーンの点に適用します。各レーンは `first` と `next_first`（残りがなければ null）を返します。番号はレーンの中の番号のままです。道具の説明に 2 つの引数を書きました。`docs/development/ai-tools.md`: `get_automation` の節を加えました。

テスト: `crates/meloet-ai/tests/link.rs` の `get_automation_gives_every_point_in_pages`（2001 個の点のあるレーン: 1 ページ目は `next_first` 2000 で終わり、2 ページ目に番号 2000 がある）。
