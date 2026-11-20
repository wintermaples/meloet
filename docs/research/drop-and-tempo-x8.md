# ファイルマネージャーからの落としこみと、x/8 拍子のテンポの数

調べた日 2026-11-21（meloet-research）。確かさは A・B・C。[S番号] は最後の表。手もとのソースは Tauri 2.12.5、tauri-runtime-wry 2.12.3、tauri-runtime 2.12.2、tauri-utils 2.10.2、wry 0.57.2（`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/`）。WebKitGTK の本物の版は 2.52.6 と聞いた（この PC の `.so` は `libwebkit2gtk-4.1.so.0.21.10`。版の番号の対応は見ていない）。GitHub や GitLab の文は、取りこみの道具が要約して返したので、要約を通している（重要な所は 2 回確かめた）。

## 結論（先に）

### 6. ファイルマネージャーからのドラッグ

1. **meloet のいまの作りは、Tauri の標準の受けかたそのまま**（A [S1] [S2]）。`tauri.conf.json` に `dragDropEnabled` は書いていない。既定は有効（A [S6]）。`crates/meloet-app/src/main.rs` の `window.on_window_event` で `WindowEvent::DragDrop` を受け、`Enter` の道を `files_over`、`Drop` の道を `files_dropped`、`Leave` を `files_left` で core へ送る。`Over` は何もしない。位置は 1 つも使っていない。画面側（`ui/src`）に Tauri のドラッグの受け口はない（Grep で 0 件）。
2. **Linux では、wry が GTK の信号 4 つにつなぐ**（A [S3]）。`drag_data_received`（`Enter`）、`drag_motion`（`Over`）、`drag_drop`（`Drop`）、`drag_leave`（`Leave`）。
3. **位置は「ウィジェットの中の、GTK の論理ピクセルの整数」**。Tauri の型は `PhysicalPosition<f64>` と書くが、wry も tauri-runtime-wry も拡大率をかけない。数をそのまま入れ替えるだけ（A [S3] [S4]）。**Linux では、数は「物理」ではない**。150 % などでも、GTK 3 の論理ピクセル（meloet の画面では CSS ピクセルとほぼ同じ。理由は下）。ほかのプロジェクトも同じ読みで直している（C [S16]）。分数の拡大率（150 %）での本当の数は、まだ測っていない。
4. **`Over` はずっと来る。ただし `Enter` のあと**。`Enter` が出るまでの動きは、位置を覚えるだけで `Over` にならない。`Enter` の位置は、`Enter` の前の最後の `drag_motion` の位置（A [S3]）。`Over` には道（paths）はない。
5. **Wayland で動くかは、「動くはずだが、meloet の本物では確かめた記録がない」**。wry は 2024-11 に「Wayland で `Drop` が来ない」を直した（0.47.0 の変更。Tauri issue #11282。A [S5] [S7]）。0.57.2 の中にその直しが入っている（A [S3]）。本物の Nautilus から落とした記録は、`docs/` に見つからなかった（Grep）。core の単体の試験（`Input::FilesDropped` など）と偽物の場面だけ。**本物の Nautilus で、オーナーさんに確かめてもらう必要がある**（手順は後ろ）。
6. **`dragDropEnabled` をオンのまま（いまのまま）で、ページの中の HTML5 のドラッグは Linux では壊れない**（A [S3] [S8]）。wry の文書が「HTML5 のドラッグが止まるのは Windows だけ」と書く。Linux は、`uri-list` を持つドラッグ（ファイルやリンクの URL）だけ wry が先に食べる。meloet の画面は HTML5 のドラッグを使っていない（`draggable`、`dragstart`、`dataTransfer` は Grep で 0 件）ので、関係ない。
7. **注意が 3 つ**（下の「落とし穴」）: 道が `file://` 以外の物、wry が食べなかった落としこみで WebKit が「そのファイルのページへ移る」おそれ（meloet に移動の見はりがない）、最初の 1 回の落としこみが Wayland で効かない古い報告。

### 7. x/8 のときのテンポの数

1. **VST3**: `projectTimeMusic`、`barPositionMusic`、`cycleStartMusic`、`cycleEndMusic` は「4 分音符の数」（1.0 が 4 分音符 1 つ）と SDK のヘッダにはっきり書いてある（A [S9] [S10]）。**`tempo` は「BPM (Beats Per Minute)」とだけで、何の音符かは書いていない**（A、見つからないことの確認）。ただし、読む側と送る側の実物は 4 分音符の数で動いている（Ardour は `quarter_notes_per_minute()` を入れる。Surge XT は `tempo` で `ppqPos` を進める。どちらも A）。**meloet の「いつも 4 分音符の数」で正しい**（実物の慣習に合う）。
2. **LV2**: 仕様の文は、「拍（beat）は `beatUnit` の音符 1 つ」「`beatsPerMinute` は拍のテンポ」と読める（A [S11]）。だから**仕様を文字どおり読むと、6/8 の `beatUnit` は 8 で、`beatsPerMinute` と `time:beat` は 8 分音符の数**になる。**実装は食いちがう**:
   - **JUCE の LV2 の包み（Surge XT）は、何も直さない**。`beatsPerMinute` をそのまま BPM、`time:beat` をそのまま ppq（4 分音符の数）、`beatUnit` をそのまま拍子の分母に入れる。`beatUnit` で割らない、4 をかけない。`ppqPositionOfLastBarStart` は入れない（A [S12]）。
   - **Ardour** は、`time:Position` の `beatsPerMinute` に「テンポの音符あたり（`note_types_per_minute`。Ardour のテンポの音符は拍子の分母とは別で、既定は 4 分）」を入れ、`beatUnit` は拍子の分母。`bpm` の control ポート（`lv2:designation time:beatsPerMinute`）には**いつも 4 分音符の数**を入れる。`time:beat` は送らない（A [S13]）。
   - **Carla** は、`beat` に「小節の中の 0 始まりの拍」を入れ（走りつづける拍ではない）、`beatUnit`、`beatsPerBar`、`beatsPerMinute` を割らずに入れる（A [S14]、要約）。
3. **meloet への当てはめ（C、推論）**: **`beatsPerMinute` は 4 分音符の数（VST3 の `tempo` と同じ値）、`time:beat` は 4 分音符の数（VST3 の `projectTimeMusic` と同じ値）、`beatUnit` と `beatsPerBar` は本当の拍子（6/8 なら 8 と 6）**。これは仕様の文字どおりとは `beatUnit` が 4 でないときにずれるが、JUCE の包み（Surge XT）が ppq とテンポを割らずに使うので、これでないと Surge XT などで位置がずれる。Ardour の既定（テンポの音符が 4 分）とも bpm が合う。

## 6. くわしい中身

### 6-1. Tauri 2 の道（A、手もとのソース）

```
GTK の信号（wry-0.57.2/src/webkitgtk/drag_drop.rs）
  → WryDragDropEvent
  → tauri-runtime-wry の with_drag_drop_handler（lib.rs 4783〜4817。座標を PhysicalPosition::new(x as _, y as _) に入れるだけ。handler は true を返す）
  → SynthesizedWindowEvent::DragDrop（窓の中身の webview）
  → tauri の WindowEvent::DragDrop（meloet-app が on_window_event で受ける）
  → 同時に、画面（JS）へ tauri://drag-enter / drag-over / drag-drop / drag-leave を emit（meloet は聞いていない）
```

| 項目 | 中身 | 確かさ |
|---|---|---|
| 設定 | `dragDropEnabled`（別名 `drag-drop-enabled`）。既定 true。「Windows で HTML5 のドラッグを使うには切る」と文書にある。`WebviewBuilder::disable_drag_drop_handler` に対応 | A [S6] |
| イベント | `DragDropEvent`（`#[non_exhaustive]`）: `Enter { paths, position }`、`Over { position }`、`Drop { paths, position }`、`Leave`。位置は `PhysicalPosition<f64>` | A [S2] |
| `Drop` のとき tauri が足す事 | `scopes.allow_file(path)` / `allow_directory`（落とした物を、fs とアセットの許可に足す）。meloet は fs プラグインを使っていないので、いまは当たらない（C、使っていないことは見ていない） | A [S4] |
| 画面（JS）へのイベント名 | `tauri://drag-enter`、`drag-over`、`drag-drop`、`drag-leave`。`paths` は `Enter` と `Drop` だけ | A [S4] |

### 6-2. Linux（wry の `drag_drop.rs`）の動き（A [S3]）

状態は `Entered`、`Leaving`、`Left` の 3 つ。

| GTK の信号 | wry がすること | handler の返り値 |
|---|---|---|
| `drag_data_received`（`info == 2` のときだけ。2 は WebKit の `URIList`） | `data.uris()` を道に直す（`file://` をはずし、% を戻す）。状態を `Entered` にして `Enter { paths, position = 覚えていた位置 }` を送る。道をしまう | なし |
| `drag_motion(x, y)` | `Entered` のときは `Over { (x, y) }` を送る。それ以外は位置を覚えるだけ | いつも false（あとの WebKit の処理も走る） |
| `drag_drop(x, y)` | 状態が `Leaving` で道が残っているときだけ、`ctx.drop_finish(true, time)`、状態を `Left` にして `Drop { paths, (x, y) }` を送り、handler の返り値を返す。Tauri の handler は**いつも true** | 条件が合えば true（WebKit に渡さない）、合わなければ false |
| `drag_leave` | 状態が `Left` でなければ `Leaving` にして、idle で `Leave` を送る | なし |

- **`Drop` が `Leaving` を条件にする理由**: GTK 3 は、落としたとき、`drag-drop` の前に `drag-leave` を出す（A [S15]。GTK のソースの要約: 「We send a leave here so that the widget unhighlights properly」）。wry は、そのあとの `drag_drop` で `Drop` にする。idle の `Leave` が先に走ると `Drop` は来ない（コードの読み。C、推論）。
- **WebKitGTK 側（2.52.6、A [S17]）**: GTK 3 の `DropTargetGtk3.cpp`。`DropTargetType { Markup, Text, URIList, NetscapeURL, SmartPaste, Custom }` で、`URIList` が 2。信号は `g_signal_connect_after` でつなぐ。wry の信号は先に走る。`drag-motion` の新しいドラッグで `accept()` が、持ち主が出している型のうち知っている物（`text/uri-list` を含む）に `gtk_drag_get_data` を 1 回ずつ呼ぶ。合う型が 1 つもなければ何もしない。そのとき wry は `Enter` を出せない。
- **`Enter` が出る時**: 最初のポインタの動きの直後ではなく、WebKit が `text/uri-list` を取りよせて、データが着いた時（コードの読み）。そのため `Over` は、その後から（それまでの動きは位置を覚えるだけ）。
- **道の直しかた**: `file://` をはずして % を戻すだけ（A [S3]）。`trash:///`、`recent:///`、`smb://…`、`sftp://…`、`file://localhost/…` のような `file:///` で始まらない物は、`PathBuf` に壊れた文字がそのまま入る（コードの読み。C、推論。Nautilus がそういう URI を出すか、本物では確かめていない）。

### 6-3. 位置（座標）

- **何の座標か（A [S3] [S15]）**: `drag_motion` と `drag_drop` の `x`、`y` は、GTK 3 が「ポインタの root 座標から、受ける窓の原点を引き、受けるウィジェットの座標に直した数」。GDK の座標は拡大率をかける前の論理ピクセルの整数。wry は `webview`（`WebView` ウィジェット）に信号をつないでいるので、**webview の左上が原点**（タイトルバーは入らない）。GTK 3 の文書は「現在のポインタの x 座標」とだけ書き、座標系までは書いていない（A [S18]）。GTK のソースの読みは要約を通している。
- **`Enter` の位置**: その前の `drag_motion` で覚えた物（整数）。覚えた物が一度もなければ `(0, 0)`。
- **Tauri の型の名前が嘘になる所**: `PhysicalPosition<f64>` に、論理ピクセルの整数をそのまま入れている（A [S4] で `x as _`）。Windows だけが本当に物理の数を出す、という報告がある（C [S16]）。
- **meloet の画面との対応（C、推論）**: meloet の 100・125・150 % は、ブラウザのズームではなく `tokens.ts` が率ごとの値を出す形（`docs/architecture.md` 221 行。`set_zoom` は `main.rs` に見つからない）。だから、ページの座標（CSS ピクセル）と GTK 3 の論理ピクセルは同じ大きさになるはず。落とした位置の x、y は、そのままページの `clientX`、`clientY` と同じ座標として `document.elementFromPoint` などに使える見こみ。**測るまでは「見こみ」**。
- **分数の拡大率（150 %）**: GTK 3 は整数の拡大率しか持たない。GNOME 50 の Wayland で 150 % のとき、窓の論理座標がどうなるか（GTK 3 の窓が整数の 2 倍の絵を出して、コンポジタが縮めるのか、論理座標の数が変わるのか）は、調べきれなかった（わからない）。meloet の窓は、論理座標の窓の大きさ（`window.scale_factor()` を使う `content_size`）を、すでに読んでいる。落とした位置を窓の大きさとくらべれば、まちがいは見つけやすい。

### 6-4. HTML5 のドラッグと、`dragDropEnabled`

- wry の文書（A [S8]）: 「handler が true を返すと OS の既定の動きを止める。そのとき `<input type="file">` に落とせなくなる。**Windows: HTML のドラッグ ・ ドロップ（`draggable="true"` など）が止まる**（WebView2 の handler を入れかえるため）」。Linux と macOS の文は書いていない。
- Linux では、信号をつなぐだけで、HTML5 のドラッグそのものは WebKit の処理のまま。wry が先に食べるのは、`uri-list` を持つドラッグ（`info == 2` が来たとき）。ページの中の普通の要素のドラッグや、選んだ文字のドラッグは、`uri-list` を持たなければ素通り（コードの読み。C、推論）。ページの中のリンクや画像のドラッグは、URL を持つので、`Enter` が出て `Drop` まで食べられうる。meloet は使っていない。
- 第 3 回に「ドラッグで文字が選ばれる」を直した記録があり（`docs/progress.md` 453 行）、画面は `user-select: none`（`ui/src/components/common.css`）。

### 6-5. 知られている問題

| 問題 | 中身 | 今の meloet への当たり | 確かさ |
|---|---|---|---|
| Wayland で `Drop` が来ない（Tauri #11282） | wry 0.44.1、webkit2gtk-4.1 2.46.1、KDE と GNOME の Wayland で `Enter`、`Over`、`Leave` は来るが `Drop` が来ない。2024-10-09 に報告、wry PR #1408 で 2024-11-09 に閉じた。「Wayland では、`Over` と `Leave` の間に道が見えない」という報告も（`Over` に道がないのは設計どおり） | 直しは 0.57.2 に入っている（`Leaving` の状態、changelog 310 行「Wayland（と X11 のときどき）で `Drop` が来ない」）。**この PC（GNOME 50、Wayland、WebKitGTK 2.52.6）での再現は報告になし** | A（issue の本文と changelog）[S5] [S7] |
| 落としたあとにドラッグの絵が残る（GNOME） | Tauri #11343。wry PR #1412 で `drop_finish` を入れた | コードに `ctx.drop_finish(true, time)` がある | A（changelog 313 行）[S7] |
| 最初の 1 回の落としこみが Wayland で効かない（WebKit bug 204281） | WebKitGTK 2.26.2、Wayland。ファイルを WebView へ直接ドラッグしても落とせず、いちど全部の窓の外（上のバーやデスクトップ）を通ってから戻ると落とせる。X11 では出ないと報告。担当者が再現できず、NEW のまま | 2019 年の報告。2.52.6 で起きるかは、わからない。**本物の Nautilus で確かめる項目に入れる** | B（2 年より古いので 1 段下げた。元は A）[S19] |
| GTK のドラッグ全体の壊れ（WebKit bug 265857） | WebKitGTK 2.42.1（GTK 4 の Epiphany）。ページの HTML5 のドラッグで `drop` が来ない。`dragstart` で `dataTransfer.setData()` を呼ぶと動く。2025-09 にも NEW。Tauri の利用者も当たった | GTK 4 の DropTarget の話。meloet は GTK 3 と wry の信号で受けるので、別の道（コードの読み。C、推論）。meloet は HTML5 のドラッグを使わない | A（bug の本文）[S20] |
| Nautilus から WebView へのファイルの落としこみ（WebKit bug 320301） | WebKitGTK 2.52.5、GTK 4.22.4、Fedora 44、GNOME 50.3 の Wayland、Epiphany 50.4 と MiniBrowser。MDN の例に Nautilus から落としても、見た目の返事がなく、離すと**ページがその画像のページに入れかわる**。原因の見こみ: `DataTransfer::allowsFileAccess()` が Cocoa 以外で false（bug 323277 に patch）。NEW | GTK 4 の話で、DOM の `drop` の話。meloet は DOM の `drop` を使わず、wry が GTK の信号で取る。**ただし「WebKit が落としこみを自分で処理すると、ページが移る」ことを示す**（下の落とし穴 2） | A（bug の本文）[S21] |

### 6-6. 落とし穴（meloet に当たりうる物）

1. **道が `file:///` 以外**（C、コードの読み）。Nautilus の「最近使ったファイル」やごみ箱、ネットワークの場所から落とすと、`PathBuf` が `smb://…` のような文字になりうる。core は存在と拡張子で見ているはずなので、「SF2 でない」の文になりやすい。見ておくとよい。
2. **移動の見はりがない**（A、Grep で `on_navigation` は 0 件）。wry が食べない落としこみ（`text/uri-list` を持たない型だけの落としこみ、上の古い報告のような最初の 1 回など）は、WebKit が自分で処理して、ページがそのファイルのページに入れかわるおそれがある（bug 320301 は GTK 4 でそれが起きた。meloet の GTK 3 で起きるかは、わからない。C、推論）。CSP の `default-src 'self'` は、ページの移動（`file://` へ）は止めない（C、推論）。もし起きたら、`WebviewWindowBuilder::on_navigation` で、アプリの URL 以外を止める案がある（作る側が決める）。
3. **ファイルをいくつも落とす**: `paths` は並びのまま全部来る。meloet の core は「SF2 はぜんぶ順に足す」（`design-notes.md` B-89）。フォルダも `paths` に入る（`tauri` は `is_file()` で分けて許可に足すだけ）。
4. **アレンジに落とす（第 7 回の音のファイル）**: 位置が要るなら、`Over` を core とやりとりして画面に出す（60 Hz で変わる物を足すと WebKit の描きなおしの決まりに当たる。`docs/architecture.md` 7 章「CPU で描く WebKit」）か、画面（JS）の `tauri://drag-over` を `ui/src/bridge/` で聞くか。作る側が決める。どちらでも数は「論理ピクセル」と同じ（6-3）。

### 6-7. 本物の Nautilus から確かめる手順（オーナーさんに頼む形）

**前提**: meloet の窓は Wayland なので、`meloet-agent` の側から本物のドラッグの出来事は作れない（X の作り物の出来事は届かない。`docs/progress.md` 1211 行）。オーナーさんのセッションの Nautilus で、手でやってもらう。

1. （係が先に）`on_window_event` の中に、一時的に `eprintln!("{drag:?}")` を足し、位置の数と `window.scale_factor()` を一緒に出す。確かめたら外す。
2. 画面の拡大率（GNOME の設定）を 100 %、200 %、150 % の順に変える。
3. Nautilus から `.sf2` を 1 つ、meloet の窓の中へドラッグし、**ポインタを窓の四隅と真ん中に止める**。見るもの: `Enter` が出る、`Over` が動くたびに出る、位置が窓の論理の大きさ（四隅の数が 0 と幅・高さのあたり）と合う、離すと `Drop` が 1 回出る、ドラッグの絵が消える、ページが移らない。
4. 同じ操作を「Nautilus の窓から meloet の窓へ、いきなり入る」形で最初の 1 回やる（bug 204281 の形）。
5. Esc で取りやめて `Leave` が出ること。窓の外へ出て `Leave` が出ること。
6. ファイルを 3 つ（`.sf2` と `.wav` と名前に日本語やスペースがある物）をまとめて。フォルダ 1 つ。
7. 「最近使ったファイル」と「ごみ箱」の中のファイルを落とす（道がどう出るか）。
8. 録音中、保存の確認ダイアログの間、プラグインの窓がひらいている間も 1 回ずつ（core が受けない形か）。

## 7. くわしい中身

### 7-1. VST3（A [S9] [S10]。ヘッダ原文）

| 欄 | 文 | 旗 |
|---|---|---|
| `tempo` | "tempo in BPM (Beats Per Minute)" | `kTempoValid` |
| `timeSigNumerator` | "time signature numerator (e.g. 3 for 3/4)" | `kTimeSigValid` |
| `timeSigDenominator` | "time signature denominator (e.g. 4 for 3/4)" | `kTimeSigValid` |
| `projectTimeMusic` | "musical position in quarter notes (1.0 equals 1 quarter note)" | `kProjectTimeMusicValid` |
| `barPositionMusic` | "last bar start position, in quarter notes" | `kBarPositionValid` |
| `cycleStartMusic`、`cycleEndMusic` | "in quarter notes" | `kCycleValid` |
| `samplesToNextClock` | "MIDI Clock Resolution (24 Per Quarter Note)" | `kClockValid` |

- **`tempo` が何の音符か**: ヘッダと公式の文書（vst3_doc、v3.8）のどちらにも書いていない（A、見つからないことの確認）。ヘッダが「4 分音符」と名指しするのは、`...Music` の欄だけ。
- **実物の慣習（A）**: Ardour の VST3 は `context.tempo = metric.tempo().quarter_notes_per_minute()`、`projectTimeMusic = quarters_at_sample(start)`、`timeSigNumerator = divisions_per_bar()`、`timeSigDenominator = note_value()`。6/8 の特別扱いはない [S22]。Surge XT は `time_data.tempo = cp.bpm`、`ppqPos = cp.ppqPosition`、`timeSigNumerator/Denominator` をそのまま入れ、ブロックごとに `ppqPos += BLOCK_SIZE * tempo / (60 * samplerate)` で進める。つまり**`tempo` は「4 分音符の数」として ppq に足されている** [S23]。JUCE の `AudioPlayHead` の文書は、`getPpqPosition()` を「4 分音符の単位」と書き、`getBpm()` と `TimeSignature` には音符の指定がない [S24]。
- **ほかの読み（C）**: Steinberg の SDK の利用者が書いたコードは `tempo / 60` を「秒あたりの 4 分音符」とし、`timeSigDenominator / 4.0` を「4 分音符あたりの拍の数」に使う [S25]。KVR の返事は「テンポは厳密には 4 分音符の数」と「6/8 は付点 4 分が 1 拍」で割れている [S26]。MuseScore の保守者も「BPM は 4 分音符の数」と書く [S26]。
- **6/8 の `barPositionMusic`**: 1 小節は 4 分音符 3 つ（4 × 6 / 8。前の調べ物 `automation-vst3.md` 94 行の計算。C）。Ardour は `(bars - 1) * 4` で 4/4 前提（A [S22]。前の調べ物と同じ）。
- **meloet の決め（いつも 4 分音符の数）の評価**: `projectTimeMusic`、`barPositionMusic`、`tempo` が全部 4 分音符の数で並ぶので、プラグインの内側の計算（`tempo` から ppq を進める）と矛盾しない。

### 7-2. LV2（仕様は A [S11]、実装は A [S12] [S13] [S14]）

仕様（`/usr/lib/lv2/time.lv2`、time 1.6）の文:

| 欄 | 文 |
|---|---|
| `time:beat` | 「小節の中の拍ではなく、`time:bar` と同じ原点から数え、頭出しがなければ増えつづける」。型 double。"The global running beat number." |
| `time:beatUnit` | 「1 拍に数える音符。拍子の下の数。2 は 2 分、4 は 4 分」。"The note value that counts as one beat." |
| `time:beatsPerMinute` | "Tempo in beats per minute."（型 float） |
| `time:beatsPerBar` | 「1 小節の拍の数」 |
| `time:barBeat` | 「小節の中の拍、0 から `beatsPerBar` まで」 |
| 全体の説明 | 「拍は音楽の時間の 1 つの脈。拍の時間は、テンポ `beatsPerMinute` で実の時間とむすびつく」 |

- **仕様だけで決まること**: 「拍」は `beatUnit` の音符 1 つ。「テンポ」は拍の数。ここから、6/8 の `beatUnit` が 8 なら `beatsPerMinute` と `beat` は 8 分音符の数、と読むのが自然（A の文からの読み。ただし文は「テンポの拍は `beatUnit` の拍」と**はっきり書いてはいない**。C、推論）。
- **JUCE の LV2 の包み（Surge XT の元。`surge-synthesizer/JUCE` の `cf5754b19c87…`、A [S12]、要約）**: `readNewInfo` で、`setBpm(beatsPerMinute)`、`setPpqPosition(beat)`、`setTimeSignature({beatsPerBar, beatUnit})`。`beatUnit` で割る行も、4 をかける行もない。`ppqPositionOfLastBarStart` を入れる行もない。コードに「Carla はいつも整数の `beat` をくれるようだが、期待したのと…」という旧い注意書きの断片がある。**JUCE の LV2 のプラグインは、`beat` が 4 分音符の数だと思っている**。
- **Ardour（A [S13]、要約。2 回確かめた）**:
  - `time:Position` の `beatsPerMinute`: `bpm = superclock_ticks_per_second()*60 / superclocks_per_note_type_at_superclock(...)`。テンポの音符（Ardour ではテンポの音符は拍子の分母と別。既定は 4 分）あたりの数。
  - `beatUnit`: `meter().note_value()`（LV2 1.10 以上。古い道は `note_divisor()`）。`beatsPerBar`: `divisions_per_bar()`。
  - `barBeat`: `bbt.beats - 1 + bbt.ticks / ticks_per_beat`。拍子の拍の単位（分子の側）。分母で割らない。
  - `bpm` の control ポート（`lv2:designation time:beatsPerMinute`）: `tmap->quarters_per_minute_at(start0)`。**いつも 4 分音符の数**。atom の `beatsPerMinute` とは、テンポの音符が 4 分でないとき食いちがう。
  - `time:beat` は送らない（前回の調べ物と同じ）。「変わったか」の判定には、`beatpos *= tempo().note_type() / 4.0` で 4 分音符へ直した数を使う。
- **Carla（A、要約。`process()` の中）**: `beat` = `bbt.beat - 1`（0 始まり。**走りつづける拍ではなく、小節の中の拍**）、`beatUnit` = `bbt.beatType`、`beatsPerBar` = `bbt.beatsPerBar`、`beatsPerMinute` = `bbt.beatsPerMinute`。`beatType` で割る行はない [S14]。
- **jalv**: 前の調べ物のとおり `beat` は送らない。`beatsPerMinute` に JACK の値を入れる。JACK の `beat_type` との関係は、今回は読んでいない（わからない）。
- **まとめ（実装の食いちがい）**: `beatsPerMinute` の音符と `beat` の単位は、実装でばらばら。JUCE の受け手は 4 分音符と見る。Ardour は atom ではテンポの音符（既定 4 分）、control ポートでは 4 分。Carla は `beat` が別物。**仕様の文字どおりに送る送り手は見つからなかった**（Ardour の `beatsPerMinute` は、テンポの音符が拍子の分母と同じとき以外は文字どおりでない）。

### 7-3. meloet への当てはめ（C、推論。入力は上の A）

| 欄 | 6/8、4 分音符 120 のとき | 理由 |
|---|---|---|
| VST3 `tempo` | 120 | 実物がそう読む（Ardour、Surge XT）。meloet の決めと同じ |
| VST3 `timeSigNumerator`、`Denominator` | 6、8 | ヘッダの例どおり |
| VST3 `projectTimeMusic`、`barPositionMusic`、`cycle*` | 4 分音符の数 | ヘッダ |
| LV2 `beatsPerMinute` | 120 | JUCE の受け手が ppq とテンポを割らずに使う。Ardour の既定とも合う。仕様の文字どおりなら 240（8 分音符の数） |
| LV2 `time:beat` | 4 分音符の数（VST3 の `projectTimeMusic` と同じ値） | JUCE が ppq として使う。仕様の文字どおりなら 8 分音符の数 |
| LV2 `beatUnit`、`beatsPerBar` | 8、6 | 拍子の本当の数。JUCE は拍子の表示と同期に使う |
| LV2 `barBeat` | 小節の中の `beatUnit` の拍（6/8 なら 0〜6 の数） | 仕様と Ardour が同じ。JUCE は読まない |
| LV2 `bpm` の control ポート（`lv2:designation time:beatsPerMinute`） | 120 | Ardour が 4 分音符の数で入れる |

- **仕様からずれる所を決めて書いておくとよい**: LV2 の `beatsPerMinute` と `beat` を「4 分音符の数」にするのは、`beatUnit` が 4 でないとき仕様の文字どおりではない（C）。かわりに、見つかった実装（JUCE、Ardour の既定）の読みかたに合う。仕様の文字どおりに読むプラグインがあれば、`beatUnit` と `beatsPerMinute` から「拍の長さ」を出すとき、6/8 では 8 分音符が 1 拍のはずなのに 4 分音符の長さで計算するので、半分の速さに見えうる（C、推論。そういうプラグインを見つけてはいない）。
- 4/4 や 3/4、2/4（分母 4）では、どの読みでもずれない。ずれるのは分母が 4 でないとき（x/8、x/2、x/16）だけ。
- 既存の前の調べ物との差（このファイルだけの新しい点）: (a) `automation-lv2.md` 69 行の「JUCE は `beat` をそのまま ppq にする」は、割る行がないことまで A で確かめた。(b) Ardour の atom の `beatsPerMinute` と control の `bpm` の単位がちがう、は新しい点（前の調べ物は同じように書いてあった）。(c) Carla の `beat` は小節の中の 0 始まり。(d) JUCE の LV2 は `ppqPositionOfLastBarStart` を入れない。

## 出どころ

| ID | URL または場所 | 何が書いてあったか | 確かさ | 見た日 |
|---|---|---|---|---|
| S1 | `/srv/meloet-agent/meloet/crates/meloet-app/src/main.rs`（383〜400 行）、`crates/meloet-app/tauri.conf.json`、`crates/meloet-core/src/service.rs`（524〜538 行）、`crates/meloet-app/src/core_handle.rs` | いまの受けかた。`DragDropEvent` の `Enter`、`Drop`、`Leave` を core へ。`Over` は無視。`dragDropEnabled` なし。`on_navigation` なし | A（手もとのコード） | 2026-11-21 |
| S2 | `tauri-runtime-2.12.2/src/window.rs`（99〜121 行）、`tauri-2.12.5/src/manager/window.rs`（205〜275 行）、`manager/webview.rs`（728〜760 行） | `DragDropEvent` の形（`PhysicalPosition<f64>`、`#[non_exhaustive]`）、JS のイベント名、`Drop` で許可に足す | A（手もとのソース） | 2026-11-21 |
| S3 | `wry-0.57.2/src/webkitgtk/drag_drop.rs`、`webkitgtk/mod.rs`（319〜322 行） | GTK の信号 4 つ、`Entered/Leaving/Left`、`info == 2`、位置の扱い、`drop_finish` | A（手もとのソース） | 2026-11-21 |
| S4 | `tauri-runtime-wry-2.12.3/src/lib.rs`（4783〜4817 行） | 座標を拡大率なしで `PhysicalPosition` に入れる、handler はいつも true | A（手もとのソース） | 2026-11-21 |
| S5 | https://github.com/tauri-apps/tauri/issues/11282 、https://github.com/tauri-apps/wry/pull/1408 | Wayland で `Drop` が来ない（wry 0.44.1、webkit2gtk-4.1 2.46.1）。2024-11-09 に直した | A（issue の本文。要約を通した） | 2026-11-21 |
| S6 | `tauri-utils-2.10.2/src/config.rs`（1978〜1988 行）、`tauri-2.12.5/src/webview/mod.rs`（1048〜1055 行）、`tauri-runtime-2.12.2/src/webview.rs`（355、521 行） | `dragDropEnabled` の既定 true、「Windows で HTML5 のドラッグを使うには切る」 | A（手もとのソース） | 2026-11-21 |
| S7 | `wry-0.57.2/CHANGELOG.md`（310、313 行） | PR #1408「Wayland（と X11 のときどき）で `Drop` が来ない」を直した。PR #1412「ドラッグの絵が残る」を直した | A | 2026-11-21 |
| S8 | `wry-0.57.2/src/lib.rs`（651〜665、1186〜1200 行） | handler が true で OS の既定を止める、`<input type=file>` に落とせなくなる、Windows だけ HTML のドラッグが止まる | A（手もとのソース） | 2026-11-21 |
| S9 | https://raw.githubusercontent.com/steinbergmedia/vst3_pluginterfaces/master/vst/ivstprocesscontext.h | `ProcessContext` の欄の文（`tempo` は BPM、`...Music` は 4 分音符） | A | 2026-11-21 |
| S10 | https://steinbergmedia.github.io/vst3_doc/vstinterfaces/structSteinberg_1_1Vst_1_1ProcessContext.html | 同じ文。`tempo` の音符の指定なし | A | 2026-11-21 |
| S11 | `/usr/lib/lv2/time.lv2/time.ttl`、`time.meta.ttl`（time 1.6） | `beat`、`beatUnit`、`beatsPerMinute`、`beatsPerBar`、`barBeat` の定義 | A（実物の仕様） | 2026-11-21 |
| S12 | https://raw.githubusercontent.com/surge-synthesizer/JUCE/cf5754b19c87ea63758802e3f4239c05a77f1412/modules/juce_audio_plugin_client/juce_audio_plugin_client_LV2.cpp | `readNewInfo` が `beatsPerMinute`、`beat`、`beatUnit` を割らずに入れる。`ppqPositionOfLastBarStart` は入れない | A（要約を通した。前回と今回で 2 回） | 2026-11-21 |
| S13 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/lv2_plugin.cc | `beatsPerMinute`（テンポの音符あたり）、`beatUnit`（拍子の分母）、`barBeat`、control ポートの `bpm`（4 分音符）。先頭の 10 万字のみ | A（要約を通した。2 回確かめた） | 2026-11-21 |
| S14 | https://raw.githubusercontent.com/falkTX/Carla/main/source/backend/plugin/CarlaPluginLV2.cpp | `process()` の `time:Position`（`beat = bbt.beat - 1` など）。10 万〜20 万字の範囲 | A（要約を通した。1 回） | 2026-11-21 |
| S15 | https://raw.githubusercontent.com/GNOME/gtk/gtk-3-24/gtk/gtkdnd.c | `drag-motion` と `drag-drop` の `x`、`y` の作りかた（root 座標から窓の原点を引き、ウィジェットの座標へ）。落としたとき、`drag-drop` の前に `drag-leave` | A（要約を通した。最後の 7 千字は読んでいない） | 2026-11-21 |
| S16 | https://github.com/scarletkc/Hatoba/issues/93 、https://github.com/armaxri/termiHub/pull/5056 、https://github.com/Alakazam-211/K2/issues/82（検索結果の要約のみ。本文は読んでいない） | 「Linux の落とした位置は拡大率をかけていない論理ピクセル」という他のプロジェクトの報告 | C | 2026-11-21 |
| S17 | https://raw.githubusercontent.com/WebKit/WebKit/webkitgtk-2.52.6/Source/WebKit/UIProcess/API/gtk/DropTargetGtk3.cpp | `DropTargetType`（`URIList` = 2）、`g_signal_connect_after`、`accept()` の `gtk_drag_get_data` | A（要約を通した） | 2026-11-21 |
| S18 | https://docs.gtk.org/gtk3/signal.Widget.drag-motion.html 、`signal.Widget.drag-drop.html` 、`signal.Widget.drag-data-received.html` | `x`、`y` は「現在のポインタの位置」と「落とした位置」。座標系の説明なし | A | 2026-11-21 |
| S19 | https://bugs.webkit.org/show_bug.cgi?id=204281 | 2019 年、WebKitGTK 2.26、Wayland。ファイルを WebView へ直接落とせず、窓の外を通ると落とせる | B（2 年より古いので 1 段下げた） | 2026-11-21 |
| S20 | https://bugs.webkit.org/show_bug.cgi?id=265857 | GTK で HTML5 の `drop` が来ない。`setData()` で動く。2025-09 も NEW | A | 2026-11-21 |
| S21 | https://bugs.webkit.org/show_bug.cgi?id=320301 | WebKitGTK 2.52.5、GNOME 50.3 の Wayland で、Nautilus から落とすとページが入れかわる。GTK 4 | A | 2026-11-21 |
| S22 | https://raw.githubusercontent.com/Ardour/ardour/master/libs/ardour/vst3_plugin.cc | `tempo` は `quarter_notes_per_minute()`、`projectTimeMusic` は 4 分音符の数、`barPositionMusic = (bars-1)*4` | A（要約を通した） | 2026-11-21 |
| S23 | https://raw.githubusercontent.com/surge-synthesizer/surge/main/src/surge-xt/SurgeSynthProcessor.cpp | `tempo = cp.bpm`、`ppqPos += BLOCK_SIZE * tempo / (60 * samplerate)` | A（要約を通した。ファイルの全部ではない） | 2026-11-21 |
| S24 | https://raw.githubusercontent.com/juce-framework/JUCE/master/modules/juce_audio_basics/audio_play_head/juce_AudioPlayHead.h | `getPpqPosition` は「4 分音符の単位」、`getBpm` と `TimeSignature` に音符の指定なし | A | 2026-11-21 |
| S25 | https://forums.steinberg.net/t/get-current-barsbeatsposition/201675/9 | 利用者のコード: `tempo / 60` を秒あたりの 4 分音符、`timeSigDenominator / 4.0` を 4 分音符あたりの拍に。投稿者が Steinberg の人かは不明 | C | 2026-11-21 |
| S26 | https://www.kvraudio.com/forum/viewtopic.php?p=3041295 、https://musescore.org/comment/591316 、https://github.com/jcsteh/osara/issues/1084（検索結果の要約のみ） | 「テンポは 4 分音符の数」という意見と「6/8 は付点 4 分が 1 拍」という意見が割れている | C | 2026-11-21 |
| S27 | `/srv/meloet-agent/meloet/docs/research/automation-lv2.md`、`automation-vst3.md`、`tempo-and-automation.md` | 前の調べ物。ここと重ならない所だけ書いた | A/B/C（元の確かさのまま） | 2026-11-21 |

2 年より古い情報: WebKit bug 204281 は 2019 年で 1 段下げた（B）。bug 265857 は 2023 年の報告だが、2025-09 にも更新があるので下げなかった。LV2 と VST3 の仕様の文は安定しているので A のまま。

## わからなかったこと・追加で調べるとよいこと

- **GNOME 50 の Wayland、150 % の拡大率のとき、`drag_motion` の `x`、`y` が窓の論理ピクセルのままか**。GTK 3 は整数の拡大率しか持たないので、窓の見かけの大きさとくらべる測りが要る。前に読んだ GTK のソースの要約には、拡大率の話がなかった。
- **この PC（WebKitGTK 2.52.6、Wayland）で、本物の Nautilus から `Drop` が来るか、来ないときの形**。手順は 6-7。
- Nautilus 50（GTK 4）が、ドラッグの持ち主として `text/uri-list` を出しているか。GTK 4 の `GdkFileList` が `text/uri-list` を出すと読んでいるが、今回はソースで確かめていない（わからない）。出していなければ `info == 2` が来ず、`Enter` も `Drop` も出ない。
- **WebKit が落としこみを自分で処理したとき、meloet のページが入れかわるか**（GTK 3 の道。bug 320301 は GTK 4）。
- 「最近使ったファイル」やネットワークの場所から落とした物の道の形（`trash:///` など）。
- LV2 の `time:Position` で、`beat` と `beatsPerMinute` を何の音符にそろえるのが正しいかの、**LV2 の作者（drobilla）の言葉**は見つからなかった。jalv と JACK の `beat_type` の関係も読んでいない。REAPER と Bitwig の送りかたは一次情報が見つからなかった。
- Surge XT 自体が `timeSigDenominator` を何に使うか（LFO の同期か）は調べていない。
- Ardour のテンポの音符（`note_type`）が 4 でないとき、LV2 の `beatsPerMinute` がそのまま 4 分音符の数でなくなる事を、実際の Ardour では動かして確かめていない（ソースの読みのみ）。
- GitHub のソースは、要約を通した物が多い。S12 と S13 は 2 回、S14 は 1 回だけ確かめた。
