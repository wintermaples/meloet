# PROTO (crates/meloet-protocol) — 後で見たい点(バグではない)

- crates/meloet-protocol/src/history.rs:485 — `StepWhat::Points` はコアが作ることはありません(コントロールの点は `StepWhat::ControlPoints` を使います)。それでも契約と `ui/src/components/history/stepText.ts:90` に残っています。
- crates/meloet-protocol/src/lib.rs:3083 — `KeyboardView.port_name`、`address`、`EndpointView.node_name`、`AudioView.forced_quantum`、`output_device`、`midi_input` は、その区分が変わるたびに送られますが、画面のどの部品も読んでいません(読むのはテストと `measure.rs` だけです)。architecture 3-2 では、これらを画面の内容に挙げています。
- crates/meloet-protocol/src/lib.rs:383 — 古い棚のコマンド `ShelfSelect`、`PlaySample`、`ToggleFavorite`、`ShelfSelectEffect` は、コアの中で `…Sound` や `ChooseEffect` の形を通してだけ届きます。画面も AI のリンクも、今では直接送っていません(コアの中だけで使う形にできます)。
- crates/meloet-core/src/service.rs:1410, 1452-1457 — キーボードの CC とピッチベンドは、すべて messages の Channel に `Event::Control` と `Event::Bend` として送られます(architecture 3-1 の項目 6 の通りです)。しかし画面で読んでいる聞き手はありません(`ui/src/gl/engine.ts:342` などは `note` でなければ戻ります)。画面はペダル、モジュレーション、ベンドをテレメトリから表示しています。使う人が出会う不具合ではありません。ただ、ホイールを少し動かすと、読まれない JSON のメッセージが毎秒数百できます。送るのをやめて architecture 3-1 を直すか、使うかのどちらかです(以前の報告 PROTO-001 でしたが、検証者が取り除きました)。
