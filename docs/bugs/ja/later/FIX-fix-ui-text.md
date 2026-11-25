# FIX-fix-ui-text — 後で見たい点(今回の掃除では直していない)

- ui/src/bridge/fakeParams.ts:192 — 偽のプラグインの `range` で、範囲内に収めた値は、両方の言語で「〜」でつながれています。コアは今、言語によって「〜」か "–" を選びます(UI-TEXT-004)。そのため、フェイクの英語の場面では、今も「〜」が出ます。
- crates/meloet-core/src/play/bounce.rs:71 — `bounce_file_name` は英語の表示名のときも「（音）」をそのまま残すため、"Piano (audio)" の作業ファイルは「Piano（音）」の名前になります。architecture 25-36 では、録音のファイル名はトラック名に従うと書かれています。
- crates/meloet-core/src/play/bounce.rs:93 — `bounce_made` が `Play::names` を渡すことを確かめるコアのテストがありません(モデルのテストは両方の言語で名前を確かめています)。
