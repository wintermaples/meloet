# PROTO (crates/meloet-protocol) — 後で見たい点(バグではない)

- crates/meloet-protocol/src/timeline.rs:3 — モジュールのコメントでは、テンポは拍子の下の数の音符で数えた拍を毎分いくつ進めるかと書かれています。CLAUDE.md と他のコードでは、拍子に関係なく毎分の四分音符の数です。契約の中にある、誤解を招くコメントです。
- crates/meloet-protocol/src/lib.rs:754 — `Command::Audition` には、画面が `AuditionOff` を送ると書かれています。しかし実際は、空の `keys` を付けた `audition` を送ります(`ui/src/components/pianoroll/audition.ts:78`)。そのため `Command::AuditionOff` を送る人はいません。
- crates/meloet-protocol/src/lib.rs:1527 — `Command::TypedKey` には、画面がキーの時刻を Rust の時計に直すのに `HelloReply::mono_us` を使うと書かれています。実際は `clockOffsetUs` を使います(`ui/src/keys/typed.svelte.ts:69`)。`HelloReply.monoUs` は、どこからも読まれていません。
- crates/meloet-protocol/src/history.rs:485 — `StepWhat::Points` はコアが作ることはありません(コントロールの点は `StepWhat::ControlPoints` を使います)。それでも契約と `ui/src/components/history/stepText.ts:90` に残っています。
- crates/meloet-protocol/src/lib.rs:3083 — `KeyboardView.port_name`、`address`、`EndpointView.node_name`、`AudioView.forced_quantum`、`output_device`、`midi_input` は、その区分が変わるたびに送られますが、画面のどの部品も読んでいません(読むのはテストと `measure.rs` だけです)。architecture 3-2 では、これらを画面の内容に挙げています。
- crates/meloet-protocol/src/lib.rs:3824 — `StatusMessage::AudioImported.skipped`(と `count`)は表示されていません。選んだファイルの一部が読めないとき、状態行には最初に取り込んだファイルの名前だけが出て、ファイルが抜けたことは知らされません。仕様 6 にその文はありません(文面は、オーナーさんに決めてもらう必要があります)。
- crates/meloet-protocol/src/lib.rs:383 — 古い棚のコマンド `ShelfSelect`、`PlaySample`、`ToggleFavorite`、`ShelfSelectEffect` は、コアの中で `…Sound` や `ChooseEffect` の形を通してだけ届きます。画面も AI のリンクも、今では直接送っていません(コアの中だけで使う形にできます)。
- crates/meloet-core/src/service.rs:1410, 1452-1457 — キーボードの CC とピッチベンドは、すべて messages の Channel に `Event::Control` と `Event::Bend` として送られます(architecture 3-1 の項目 6 の通りです)。しかし画面で読んでいる聞き手はありません(`ui/src/gl/engine.ts:342` などは `note` でなければ戻ります)。画面はペダル、モジュレーション、ベンドをテレメトリから表示しています。使う人が出会う不具合ではありません。ただ、ホイールを少し動かすと、読まれない JSON のメッセージが毎秒数百できます。送るのをやめて architecture 3-1 を直すか、使うかのどちらかです(以前の報告 PROTO-001 でしたが、検証者が取り除きました)。
