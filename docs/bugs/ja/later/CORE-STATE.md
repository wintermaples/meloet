# CORE-STATE: 後で見たい点(報告ではない)

- crates/meloet-core/src/state/recovery.rs:239-245 — 「戻さない」を押した後、または保存された曲まで取り消した後は、未保存の変更がなくなります。そのため `write_recovery` は何も書かず、ファイルを消す処理もありません。その後にクラッシュや PC のシャットダウンがあると、使う人が断った曲が次の起動で戻ってきます(architecture 15 は、ファイルがいつ消えるかを書いています。テスト `dont_restore_is_the_second_step_and_undo_goes_back_through_both` は、取り消しのためにファイルを残すことを意図しています)。
- crates/meloet-core/src/service/input.rs:362-372 — 保持されたボーカルのノート(`work/rec.json`)は、回復ファイルの書き込みが応答するとすぐ消されます。書き込みに失敗した場合も同じです(recovery.rs の `writer_loop` は、どちらの場合も `done` を返します)。書き込みが失敗すると、そのクリップがどこに属するかを知っているのはメモリだけになります。
- crates/meloet-core/src/service.rs:430-434 — `Held::release_all`(キーボードが抜かれた、または MIDI 入力が止まった)は、鍵とサスティンを離しますが、ベンドとモジュレーションは残します。次の録音の `Started { held }`(`Held::controls`)は、もうないキーボードのモジュレーションホイールやベンドを、テイクの先頭に書き込んでしまうことがあります。
- crates/meloet-core/src/hub.rs:105-109 — 画面が読み直されると(「ことば」、Web プロセスが戻ったときなど)、新しいシンクが登録されます。古いシンクは、送信が失敗するまで hub に残ります。失敗するかどうかは、消えたページに対して Tauri の Channel が何をするかによります(meloet-app)。
- crates/meloet-core/src/state/updates.rs:34-45 — 「新しい版をたしかめる」を同じ実行の中で切ってまた入れると、見つけた版を忘れ、次の起動まで確かめ直しません(「一回の実行で確かめるのは一度まで」には合いますが、「meloet について」の行が失われます)。
- crates/meloet-core/src/state/updates.rs:66-76 — `settle_updates` は、`handle` の残りが通知を設定する前に状態行を読むため、表示されていない行について `said` が true になることがあります。その場合、曲が動くと、その行は実行の間ずっと消されます(まれ)。
- crates/meloet-core/src/state.rs:1297-1302 — 録音中やカウントイン中に窓へ落とされたファイルは、状態行に何も出さずに飛ばされます(直前には状態行にドラッグの行が出ていました)。
- crates/meloet-core/src/state/songs.rs:377-383 — 保存の窓で、`.meloet` と空白を取り除くと空になる名前は、何もせず、何も知らせません。
