# CORE-STATE: 後で見たい点(報告ではない)

- crates/meloet-core/src/hub.rs:105-109 — 画面が読み直されると(「ことば」、Web プロセスが戻ったときなど)、新しいシンクが登録されます。古いシンクは、送信が失敗するまで hub に残ります。失敗するかどうかは、消えたページに対して Tauri の Channel が何をするかによります(meloet-app)。
- crates/meloet-core/src/state/updates.rs:66-76 — `settle_updates` は、`handle` の残りが通知を設定する前に状態行を読むため、表示されていない行について `said` が true になることがあります。その場合、曲が動くと、その行は実行の間ずっと消されます(まれ)。
- crates/meloet-core/src/state.rs:1297-1302 — 録音中やカウントイン中に窓へ落とされたファイルは、状態行に何も出さずに飛ばされます(直前には状態行にドラッグの行が出ていました)。
- crates/meloet-core/src/state/songs.rs:377-383 — 保存の窓で、`.meloet` と空白を取り除くと空になる名前は、何もせず、何も知らせません。
