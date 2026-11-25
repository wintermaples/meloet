# CORE-STATE — 2 回目で直した点 (2026-11-25)

- crates/meloet-core/src/state/recovery.rs:239-245 — 「戻さない」を押した後、または保存された曲まで取り消した後は、未保存の変更がなくなります。そのため `write_recovery` は何も書かず、ファイルを消す処理もありません。その後にクラッシュや PC のシャットダウンがあると、使う人が断った曲が次の起動で戻ってきます(architecture 15 は、ファイルがいつ消えるかを書いています。テスト `dont_restore_is_the_second_step_and_undo_goes_back_through_both` は、取り消しのためにファイルを残すことを意図しています)。
- crates/meloet-core/src/service/input.rs:362-372 — 保持されたボーカルのノート(`work/rec.json`)は、回復ファイルの書き込みが応答するとすぐ消されます。書き込みに失敗した場合も同じです(recovery.rs の `writer_loop` は、どちらの場合も `done` を返します)。書き込みが失敗すると、そのクリップがどこに属するかを知っているのはメモリだけになります。
- crates/meloet-core/src/service.rs:430-434 — `Held::release_all`(キーボードが抜かれた、または MIDI 入力が止まった)は、鍵とサスティンを離しますが、ベンドとモジュレーションは残します。次の録音の `Started { held }`(`Held::controls`)は、もうないキーボードのモジュレーションホイールやベンドを、テイクの先頭に書き込んでしまうことがあります。
- crates/meloet-core/src/state/updates.rs:34-45 — 「新しい版をたしかめる」を同じ実行の中で切ってまた入れると、見つけた版を忘れ、次の起動まで確かめ直しません(「一回の実行で確かめるのは一度まで」には合いますが、「meloet について」の行が失われます)。
