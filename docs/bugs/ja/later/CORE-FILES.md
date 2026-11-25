# CORE-FILES — 後で見たい点(バグではない)

- crates/meloet-core/src/recent.rs:30 — 起動のときマウントされていないディスクにある曲は、次の書き込みで一覧から完全に消えます。architecture 25-7 は、開けなかったときだけについて述べています。
- crates/meloet-core/src/sounds/fetch.rs:94 — 好ましい版がないときの `version_dir` は、フォルダ名が一番大きいものを選びます。版は内容のハッシュなので、古い版を二つ残しているとき、それが一番新しいとは限りません。
- crates/meloet-core/src/maker.rs:528 — `make_now` は `<key>.export-<pid>.wav` を書きますが、これは `.part` ではありません。書き出しが途中で止まると、`trim_cache` が取り除くまでそのファイルが残ります。
