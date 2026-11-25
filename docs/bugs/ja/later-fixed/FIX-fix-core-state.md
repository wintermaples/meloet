# FIX-fix-core-state — 2 回目で直した点 (2026-11-25)

- crates/meloet-core/src/songfile.rs `folder_name` — 曲のフォルダとファイルの名前は、曲の名前からバイトの上限なしで作られます。曲の名前が日本語で約 80 文字を超えると、最初の保存と「名前をつけて保存…」が失敗します(ENAMETOOLONG)。CORE-EXPORT-003 と同じ不具合です。`export::job::cut_name` を使えます。
- crates/meloet-core/src/export.rs — 書き出しは、「えらぶ…」を一度も使わなかったとき(既定の場所と曲の名前)、場所を選んだあとで形式を変えたとき(`.wav` について尋ねたのに、`.flac` に上書きする)、選んでから「書き出す」までの間にファイルが現れたときに、尋ねずに上書きします。「書き出す」で尋ねるには、新しい文面が必要です。
- crates/meloet-core/src/state/songs.rs `close_requested` — 曲を開く処理を読んでいる途中(`Waiting::Read`)や、PC の開く窓・フォルダの窓が出ているときに閉じても、今も何も言わずに捨てられます。保存のときだけ残ります(CORE-STATE-003 は保存についてのものでした)。
