# PLUG — 2 回目で直した点 (2026-11-25)

- crates/meloet-plughost/src/control.rs:217-224 — `close_window` は、スロットの保留中のパラメータを手放す前に `told` を消します。そのため、33 ms の上限で保留された値は、`ParamEnd` の前に伝えられずに捨てられます(wire.rs:399-400 は、`ParamEnd` の前の最後の値は必ず伝えられると書いています)。最後の動きから一回の確認の間に、閉じるか、アンロードが起きる必要があります。
- crates/meloet-plughost/src/control.rs:352-365 — 使う人が窓の × で閉じるとき、中にいた場合は `EditorLeft` を送りますが、`look_at_focus`(404-406)が行う `controller.editor_left()` を呼びません。そのため、この閉じ方では、離れた後の VST3 の再同期(architecture 20-14)が行われません。その再同期が必要なプラグインは、まだ見ていません。
- crates/meloet-plugin/src/ipc/ends.rs:211-216 — `block_ms` は整数のミリ秒です。そのため、1 ms 未満のブロック(48 kHz の 32 フレーム)では 0 になり、ホストのスレッドが持つ要求はどれも長いと数えられます。期限を過ぎた応答を一度も眠らせずに待つことになります(ブロックごとに一回ずつ、無音で)。
- crates/meloet-engine/src/plugin.rs:198-208 — `push_event` が false を返しても無視しています。ホストがブロックに対して大きく遅れると(プラグインが一秒以上かけてブロックの中でパッチを読み込む場合など)、256 件のリングが一杯になり、後のイベント(ノートオフを含む)が失われます。ノートが鳴りっぱなしになる可能性があります。
- crates/meloet-plughost/src/window.rs:297-298 — X の接続が切れると(Xwayland がなくなると)、`events` は読むのを止めますが、fd は `poll` に残り続けて切断を知らせ続けます。そのため、ホストのメインループが空回りします。
- crates/meloet-plughost/src/control.rs:1149-1152 — 返信の本文が 16 MiB を超えるか、blob が 256 MiB を超えると、何も書かれる前に `write_frame` が失敗し、その後ホストが終わります。`Error` の返信にすれば、プラグインは動き続けられます。
- crates/meloet-plugin/src/scan.rs:148-149 — バンドルの中の読めないフォルダが一つあると、`BundleStamp::read` が失敗します。すると `places_todo`(scanner.rs:270)は、そのバンドルを何も言わずに二度と走査しません。
