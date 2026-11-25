# EDGE — 2 回目で直した点 (2026-11-25)

- crates/meloet-app/src/single.rs:30-32,222-242 — 後から起動した meloet は、答えを待つのに `TALK`(2 秒)しか待ちません。一つ目は、`Core::start`(main.rs:518-533)の後で初めて聞き取りのスレッドを起動します。一つ目の起動中に二つ目が起動すると、`NoAnswer` を受けて exit(1) するかもしれません。すると一つ目は、渡された曲を捨てます(`ok` の書き込みが失敗するため。single.rs:286)。
- crates/meloet-app/src/pcwindow.rs:136-142 — 曲のファイルの絞り込みは `*.meloet`(小文字)だけを受け付けます。一方、SF2、音声、画像の絞り込みは、大文字小文字の両方を受け付けます(`patterns`)。
