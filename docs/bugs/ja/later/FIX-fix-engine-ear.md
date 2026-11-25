# FIX-fix-engine-ear — 後で見たい点(バグではない)

- crates/meloet-engine/src/engine.rs:2476-2480 (`record_here`) — 再生が始まってから出口の遅れの間(ENGINE-001 からはマスターのインサートの遅れを足した分)に R を押すと、クロックにはまだ再生の区間がないため、`from` は `t.tick()` に戻ります。これは耳に届いた位置より、最大でその遅れの分だけ先です。同じ場合、`run_place` は `segment_now(0)` から数え戻します。
- crates/meloet-engine/src/engine.rs:2833-2835 (`LapChanged::t_us`) — この時刻は、その周回の最初のフレームが DAC に届いた時刻で、その曲が耳に届いた時刻ではありません(マスターのインサートの遅れは足していません)。今はコアが読んでいません。
- crates/meloet-engine/src/engine.rs:2662-2666 (`LatencyRecord`) — 設定に出る、鍵から音までの数字には、マスターのインサートの遅れが入っていません。
- crates/meloet-engine/src/engine.rs:2421-2455 (`stop`) — 止めたあと、再生の線は、最後のブロックが DAC に届いた位置(今の時刻にマスターのインサートの遅れを足したもの)まで進み、それから止めた位置へ戻ります。出口の遅れの分だけ、ENGINE-001 の前からも戻っていました。
- crates/meloet-engine/src/engine.rs:1816 (with 2180) — マスターのインサートの遅れは、コマンドを読んだあとで読まれるため、それが変わったブロックの `read_commands` の中では古い値が使われます。そのブロックでテンポの入れかえのときにクリックされた拍は、古い遅れを待ちます(古い 0 なら、すぐに鳴って動きません)。`run_place` の予備の `segment_now` も、古い遅れで数え戻します。一つのブロックだけの問題です。
- crates/meloet-engine/src/clock.rs:312 (`heard_time_of_frame`) — 残している区間のどれより古いフレームには、マスターの遅れが付きません。一方、`time_of_frame` は一番古い区間から数えます。
