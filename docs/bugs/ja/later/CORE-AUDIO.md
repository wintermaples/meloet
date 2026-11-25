# 後で見たい点: CORE-AUDIO

- crates/meloet-core/src/audiofiles.rs:429-433 — 同じ uid の表の項目が二つある場合（同じバイト列が別の名前で取り込まれている場合）、最初の名前でしか開きません。その最初のファイルがないとき、もう一方は試されません。`watch` も uid を最初の名前でしか報告しません。
- crates/meloet-core/src/readahead.rs:325-340 — 一回目の splice から 10 ms 以内に二回目の splice が来ると、二回目は一回目の新しい音から始まります。そのため、その前のフレーム（まだ古い音）が、重なりなしで新しい音へ移ります。
- crates/meloet-core/src/vocalrec.rs:1000-1013 (service/input.rs:413) — `DiskWriter::mark` は、`from` より後のブロックがまだ来ていないとき、コアのスレッドを最大 300 ms（`MARK_WAIT`）待たせます（出力の遅れが大きく、テンポが速いときに再生中の R で起きます）。
