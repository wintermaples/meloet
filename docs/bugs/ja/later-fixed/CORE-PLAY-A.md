# CORE-PLAY-A — 2 回目で直した点 (2026-11-25)

- crates/meloet-core/src/play.rs:1414-1417 — 手（例: 「トラックを足す」や「トラックを複製」の取り消し）によって選ばれているトラックがなくなると、選択は最初のトラックに移ります。一方、`delete_track`（play/tracks.rs:24-27）は、その下のトラックに移します。二つの規則は異なります。
- crates/meloet-core/src/play.rs:2183-2186 — 歌の録音がディスク書き込みを待っている間（`state` は `Stopped`、`rec` はまだ `Some`）も、`can_record` は true のままです。その短い間、R と Space は、ボタンが使えそうに見えるのに何もしません。
- crates/meloet-core/src/play/recent.rs:400-413 — 棚が音を試し弾きしている（「ためし弾き」）間に押された鍵も、鍵の対象トラックのフレーズに入ります。そのため、「さっきのを残す」が、別の楽器で聞いたフレーズを残すことがあります。
- crates/meloet-core/src/play/timeline.rs:437-443 — `go_to_marker` は、カウントイン中や録音中でもマーカーを選びます（`locate` は動くことを拒みます）。マーカーは選ばれますが、再生の線は動きません。
