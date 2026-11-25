# FIX-fix-engine-ear — 2 回目で直した点 (2026-11-25)

- crates/meloet-engine/src/engine.rs:2476-2480 (`record_here`) — 再生が始まってから出口の遅れの間(ENGINE-001 からはマスターのインサートの遅れを足した分)に R を押すと、クロックにはまだ再生の区間がないため、`from` は `t.tick()` に戻ります。これは耳に届いた位置より、最大でその遅れの分だけ先です。同じ場合、`run_place` は `segment_now(0)` から数え戻します。
- crates/meloet-engine/src/clock.rs:312 (`heard_time_of_frame`) — 残している区間のどれより古いフレームには、マスターの遅れが付きません。一方、`time_of_frame` は一番古い区間から数えます。
