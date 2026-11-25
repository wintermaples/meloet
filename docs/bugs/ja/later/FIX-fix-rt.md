# FIX-fix-rt — 後で見たい点(修正中に見つけたが、直していない)

- crates/meloet-plughost/src/formats/vst3.rs:1085-1095 — `Vst3Processor::reset` は、ゲートが取られているとき(バスを切りかえている最中)は何もしません。そのあとで行うものもないため、その瞬間に頼まれたリセットは失われます。LV2 は、リセットを `quiet` に保ち、次に動くブロックまで持ち越します。
- crates/meloet-plughost/src/formats/lv2.rs:1523-1528 — `midi` は、アトムのバッファがいっぱいのとき、イベントを捨てます(`push_midi_event` の結果は確かめていません)。同じポートに `patch:Set` とウィンドウのメッセージが多いと、理屈の上では、ブロックの終わり近くのノートオフが失われるかもしれません(MIDI だけの 512 イベントは 16 KiB のバッファに入ります)。
