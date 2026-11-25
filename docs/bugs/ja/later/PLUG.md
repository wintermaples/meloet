# PLUG — 後で見たい点(バグではない)

- crates/meloet-plughost/src/formats/vst3.rs:987-997, 1167-1176 — 使う人が VST3 の窓で動かした値(`performEdit`)は、その区切りの後に `setParamNormalized` でコントローラへ戻ります。プラグイン自身の窓でドラッグ中、プラグインによっては古い値の反響でつまみが一瞬戻ることがあります。まだ見ていません。
- crates/meloet-plughost/src/rt.rs:135-143, 156-164 — 一ブロックで、メインスレッドからの変更は最大 512 件までしか受け取りません(`to_plugin_events` と `COMMANDS`)。窓の中で一度に多くのポートを書く LV2(窓自身が読み込むプリセットなど)は、残りを失います。DSP には届かないのに、`Shared::values` には表示されます。
- crates/meloet-plughost/src/control.rs:171 — `Load` は、マッピングより大きい `max_frames` を拒みますが、小さい値は受け付けます。一方、エンジンのブロックはマッピングの大きさまで使うため、アクティベーションで決めた VST3 のバッファが溢れる恐れがあります。コアは今のところ、常にマッピングの大きさを送っています。
