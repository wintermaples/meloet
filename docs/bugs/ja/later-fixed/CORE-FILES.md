# CORE-FILES — 2 回目で直した点 (2026-11-25)

- crates/meloet-core/src/songfile.rs:1981 — 一時の名前 `.{name}.new` と `.{name}.prev.new` は、曲のファイル名に最大 10 バイト足します。`folder_name` には長さの上限がありません。そのため、約 245 バイト(日本語で約 80 文字)の曲の名前では、最初の保存は成功しますが、それ以後の保存はすべて失敗します(ENAMETOOLONG)。
- crates/meloet-core/src/songfile.rs:1985 — 前回から変わっていない保存では、今のファイルが `.prev` の上にコピーされるため、本当の前のバージョンが失われます(設計の問い: バイトが同じなら `.prev` を変えるべきか)。
- crates/meloet-core/src/songfile.rs:107 — 戻りの区分や、形の違うマスター(例: 戻りの `pan` が 12.5)があると、マスターのプラグインのインサートとその状態を含むミキサー全体が、何も言わずに捨てられます。コメントではそう意図されていますが、マスターの `pan` には寛容な読み手(`pan_or_middle`)があり、戻りの方にはありません。
- crates/meloet-core/src/settings.rs:403 — 一つの項目の型が違うと(例: `"scale": "x"`)、すべての設定(出口、お気に入り、プラグインの場所)がリセットされ、次の保存で初期値がファイルに書かれます。テストでは意図されていますが、見直す価値があります。
- crates/meloet-core/src/settings.rs:549, recent.rs:66, presets.rs:216, updates.rs:235, sounds/fetch.rs:869 — 名前の変更のあとに、フォルダの同期がありません(曲のファイルと回復のファイルは、フォルダを同期します)。
- crates/meloet-core/src/sounds/fetch.rs:664-679 — `Range` のない要求に 416 か、`Content-Range` のない 206 を返すサーバーでは、`part` が止まるまで休みなく繰り返します(`Try::Again` には回数の上限がありません)。GitHub はこうしません。
