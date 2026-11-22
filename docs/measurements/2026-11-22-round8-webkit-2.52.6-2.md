# 第 8 回 区切り2: この PC の WebKitGTK（2.52.6-0ubuntu0.26.04.2）は、どちらの道で描いているか（2026-11-22、k8ub）

## 条件

- WebKitGTK: `libwebkit2gtk-4.1-0` 2.52.6-0ubuntu0.26.04.2（2026-11-21 23:41 に `.1` から上がった。`/var/log/dpkg.log`）。BuildID `e94c7901…`。
- debug 情報: debuginfod.ubuntu.com には、この BuildID がまだなかった（`not found`）。`http://ddebs.ubuntu.com/pool/main/w/webkit2gtk/libwebkit2gtk-4.1-0-dbgsym_2.52.6-0ubuntu0.26.04.2_amd64.ddeb` を取って `dpkg-deb -x` し、`.debug` を `~/.cache/debuginfod_client/e94c7901…/debuginfo` に置いた（`webkit-damage.sh` ・ `webkit-stacks.sh` がそのまま読む）。
- アプリ: release、1920×1080、`app-watch.sh` の起動（gdb の下か、なし）。曲は `make_song.py` の `plain`（1 本）、`band8`（8 本）、`band8-vol`（8 本 ・ 音量の帯）、`band8-all` ・ `band16-all`（どのトラックにも音量 ・ パン ・ センド A ・ B の帯、点 97）。
- **PC は静かでなかった**: ほかに 3 人の係が動いていた（ビルド、CPU に負荷をかける試験 `yes` × 数十）。load は 2.5〜44。コマの時間の数は、この条件のにごった数（下の表に load を書いた）。四角の並びは load によらない。
- 「前」は foundation 8bfe372 のまま、「後」は区切り2 の画面の直し（36-B ・ 37-A ・ 38-B ・ 92 の残り）を入れた物。

## 1. 答え: 合成の層の道（LayerTreeHost ・ ThreadedCompositor ・ Skia）で描いている

- `tools/measure/webkit-damage.sh`（再生中、50 秒近く gdb の下）: `DrawingAreaCoordinatedGraphics::display()` ・ `setNeedsDisplayInRect` ・ `WebPage::drawRect` は **1 回も呼ばれなかった**（`paints: {}`）。CPU で描く道（`.1` で見ていた `shouldPaintBoundsRect` の道）は通っていない。
- WebKitWebProcess のスレッドに `ThreadedCompositor`（comm は `eadedCompositor`）と `SkiaGPUWorker` × 2 がある。NVIDIA の EGL（`libEGL_nvidia.so.595.99.02`）を読みこんでいる。
- gdb で合成の道の関数を数えた（スクラッチの道具 `k8ub_layers.py`。`LayerTreeHost::updateRendering`、`ThreadedCompositor::renderLayerTree`、`SkiaPaintingEngine::record`）: 再生中 1500 回の止まりで、`updateRendering` 500 ・ `renderLayerTree` 500 ・ `record` 500（1 コマに 1 回ずつ）。
- **層は 1 枚だけ**（`record` の層のアドレスはいつも同じ）。WebGL の canvas も別の層にならず、その 1 枚の中に描かれる（`masterMeter` の 1437×36 がそのまま `record` の四角に出る）。
- **1 コマに `record` は 1 回で、四角は「そのコマによごれた物ぜんぶを囲む四角」1 つ**:
  - `plain` 再生（アレンジだけ）: 234×20（位置の箱）372 回、1437×36（マスターの面）75 回、237×20 52 回、608×44 1 回。前（`.1`）の「3 個 ・ 2 個 …、囲む四角 60 回はどれも小さい」と同じくらいの広さ。
  - `band8` 再生 ＋ ミキサー: 728×922（197 回）、616×922（186 回）、1675×922（50 回）、1293×922（38 回）、1437×938（15 回）、1819×938（12 回）。**上の帯の数字と下のミキサーが同じコマに変わると、毎コマ窓の高さほどの四角になる**。前（`.1` の CPU の道）は、同じ場面で「囲む四角」は 0 回（いちばん大きい四角でも 554×264）。
- `WEBKIT_DISABLE_COMPOSITING_MODE=1` を外から決めると、CPU で描く道にもどる（`webkit-damage.sh` の `display()` が出る）。下の 2 章の前と後のくらべは、この変数で前の道にもどして取った。

### コマの時間（gdb なし、`app-watch.sh --seconds 25`、ページの 6〜23 秒。load は 1 分の値）

| 場面 | 道 | load | busyMs 平均 / p95 / 最大 | コマの間 p95 / 25 ms をこえた数 |
|---|---|---|---|---|
| `plain` 再生（前） | 合成（`.2` のまま） | 6.2 | 1.0 / 2 / 3 | 17 / 0 |
| `plain` 再生（前） | CPU（変数で） | 10.2 | 2.79 / 6 / 14 | 17 / 0 |
| `band8` ＋ ミキサー（前） | 合成 | 4.4 | 12.06 / 21 / 55 | 24 / 44 |
| `band8` ＋ ミキサー（前） | CPU | 9.9 | 14.55 / 40 / 97 | 32 / 91 |
| `band8-all` ＋ ミキサー（前） | 合成 | 16.8 | 21.39 / 33 / 62 | 33 / 181 |
| `band8-all` ＋ ミキサー（前） | CPU | 16.3 | 13.25 / 19 / 47 | 20 / 21 |
| `plain` 再生（後） | 合成 | 15.9 | 2.55 / 9 / 66 | 19 / 25 |
| `band8` ＋ ミキサー（後） | 合成 | 14.4 | 12.57 / 34 / 104 | 39 / 101 |
| `band8-all` ＋ ミキサー（後） | 合成 | 13.3 → 10.1 | 25.64 / 48 / 211 → 12.78 / 15 / 18（同じ場面の 2 回） | 52 / 285 → 18 / 0 |
| `band8-all` ＋ ミキサー（後） | CPU | 14.3 | 10.22 / 12 / 83 | 17 / 2 |

- 前の測り（`.1`、静かな PC）: `plain` busyMs 平均 1.88、p95 5（`2026-11-21-round7-step1.md`）。`band8` ＋ ミキサーは 6 ms ほど（第 5 回 ・ 第 6 回）。
- **読めること**: アレンジだけなら、合成の道のほうが軽い（1.0 ms と 2.8 ms）。ミキサーをひらくと、どちらも 12〜15 ms で、前の静かな PC の 6 ms より重い。同じ場面の 2 回で 25.6 と 12.8 ms ほどゆれるので、合成の道と CPU の道のどちらが速いかは、この PC が静かなときに取りなおさないと決められない（**宿題**）。
- 合成の道でも 1 コマの仕事（`record`）は主スレッドで、描く（`replay`）のは `SkiaGPUWorker`。囲む四角が大きいほど `record` が重くなるかは、gdb の止まりの数では分けられない（`webkit-stacks.sh` の割合で見るのがよい。今回は取っていない）。

## 2. 区切り2 の直しの、描きなおしの四角（CPU の道。`WEBKIT_DISABLE_COMPOSITING_MODE=1`、gdb の下。回数ではなく並びを読む）

| 場面 | 前 | 後 |
|---|---|---|
| `band8` ＋ ミキサー（帯なし） | 5 個 71、7 個 31、8 個 14、3 個 10、6 個 5、4 個 1。囲む四角 0 | （帯がないので変わらない。取っていない） |
| `band8-vol`（音量の帯）＋ ミキサー | 5 個 101、6 個 14、7 個 5、8 個 3、3 個 3、2 個 1。囲む四角 0 | 6 個 86、8 個 10、7 個 10、9 個 7、10 個 1。囲む四角 0 |
| `band8-all` ＋ ミキサー | 6 個 61、8 個 25、7 個 12、9 個 4。囲む四角 1 | 7 個 38、9 個 37、10 個 12、8 個 2。囲む四角 1 |
| `band16-all` ＋ ミキサー | 6 個 39、8 個 33、9 個 15、7 個 5、10 個 1、2 個 1。囲む四角 0 | 9 個 42、7 個 21、10 個 17、8 個 1、1 個 1。囲む四角 1 |
| `plain` ＋ 歌のトラック（マイクの計が動く）再生 | 6 個 55、3 個 30、7 個 23、5 個 22、囲む四角 22、… | 6 個 77、7 個 26、5 個 12、10 個 6、囲む四角 6、… |

- **92 の残り（ミキサーのフェーダーの値の字を毎コマ）**: 1 コマに四角が 1 つふえた（その半分の canvas を出さないコマの、値の字の行。となりのストリップとつながって 1 つ）。上限 10 個までの余りが 1〜2 になった。`band16-all` で囲む四角が 99 回に 1 回出た（`0 941 1728 27`（値の字の行）が 11 個目）。
- 一度ためした形（パンとセンドの値の字も毎コマ）は、`band8-all` で囲む四角が 126 回中 126 回 → 帯の見出しの字と同じ書きかえにまとめたせい（Region がこみ入る）と分かり、見出しのあとの次の小さな仕事に分けて 73 回中 18 回。まだ多いので、**毎コマにしたのはフェーダーの値の字だけ**にした（パンとセンドの字は、つまみと同じ 1 秒 15 回のまま）。
- **38-B（マイクの計）**: 線と目盛は同じ canvas の中の絵だけで、HTML はふやしていない。四角の並びは前と同じ形（位置の箱、マスター、入力計の区画）。囲む四角は前も出ていた（22 回。第 7 回の終わりの測りの「まるごと 3 回」と同じ種類）。後で少ないのは、load がちがう 2 回の差で、直したからではない。
- **合成の道（いまのこの PC）では、上の決まり（11 個 ・ 25 %）は効かない**。1 コマに 1 つの囲む四角なので、値の字を毎コマにしても四角の数はふえない（広さは、すでに窓の高さほどある）。

## 3. architecture 7 章で古くなった所（7 章は書きかえていない。棟梁が決める）

- 「CPU で描く WebKit」の頭の「NVIDIA ＋ Ubuntu の WebKitGTK では、合成の層がなく、ページは CPU で描かれる」は、`.1` までの話。`.2` では合成の層（1 枚）で描く。`CLAUDE.md` の「この PC の WebKitGTK は、合成の層なしでページを CPU で描く（`WEBKIT_FORCE_COMPOSITING_MODE=1` などの変数でも変わらない）」も同じ。
- 「四角が 11 個以上か 25 % 以上で囲む四角」「`cover` ・ `SHOWN_WITH` で四角を 1 つにまとめる」「レイアウトでためる Region」は、合成の道では、そのままの形では効かない。合成の道では、そのコマによごれた物ぜんぶを囲む四角 1 つを、1 枚の層に `record` する。だから「同じコマに、上の帯と下のミキサーを変えない」が、新しい道で効く形になる（いまの `SHOWN_WITH` は、ミキサーを出すコマにマスターも出すので、その逆）。
- 「canvas を出すと、その canvas ぜんたいがよごれた四角になり、GPU から 2 回読みもどす」は、合成の道でどうなっているか見ていない（canvas は層にならず、同じ層の `record` に入っていた）。
- `tools/measure/webkit-damage.sh` は、`.2` では何も出さない（`display()` が呼ばれない）。いまの道で使うには `WEBKIT_DISABLE_COMPOSITING_MODE=1` を足すか、`SkiaPaintingEngine::record` の四角を数える道具が要る（今回はスクラッチの `k8ub_layers.py` で数えた。`tools/` には入れていない）。
- 「合成の層がある PC（Intel、AMD）では … 確かめてはいない」の段は、この PC でも合成の道になったので、そのまま「この PC の今」の話になった。

## 読んだ所

- `docs/research/webkitgtk-2404-2604.md`、`docs/architecture.md` 7 章、`docs/measurements/2026-11-21-round7-step1.md` ・ `2026-11-21-round7-end.md` ・ `2026-11-20-round6-fix-redraw.md`。
- WebKitGTK 2.52.6-0ubuntu0.26.04.2 の記号表（`nm -C` の debug 情報）: `WebKit::LayerTreeHost::*`、`WebKit::ThreadedCompositor::renderLayerTree`、`WebCore::SkiaPaintingEngine::{paint,record}`、`WebKit::DrawingAreaCoordinatedGraphics::{display,setNeedsDisplayInRect}`。`record` は値を返すので、引数は 1 つずれる（層は `rdx`、四角は `rcx`）。
- `tools/measure/{webkit-damage.sh,webkit_damage.py,webkit_damage_summary.py,webkit-stacks.sh,app-watch.sh,frame_stats.py,make_song.py}`。

生の数: `target/k8ub/`（`damage-*`、`layers-*`、`frames-*`、`watch-*`。作業ツリーの外ではなく、係の作業ツリーの `target/`）。
