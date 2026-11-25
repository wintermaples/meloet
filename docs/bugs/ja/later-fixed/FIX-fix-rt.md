# FIX-fix-rt — 2 回目で直した点 (2026-11-25)

- crates/meloet-plughost/src/formats/vst3.rs:1085-1095 — `Vst3Processor::reset` は、ゲートが取られているとき(バスを切りかえている最中)は何もしません。そのあとで行うものもないため、その瞬間に頼まれたリセットは失われます。LV2 は、リセットを `quiet` に保ち、次に動くブロックまで持ち越します。
