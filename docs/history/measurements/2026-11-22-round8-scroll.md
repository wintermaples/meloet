# 第 8 回 区切り4: CPU の道のアレンジの横の送り ・ 拡大（2026-11-22、meloet-hard ・ Opus 5.5、印 k8sc）

決めた事は `docs/architecture.md` 23-13。スクラッチと生の数は、係の作業ツリーの `target/k8sc/`（`m/` に回ごとの `summary.json`、`k8sc-*.sh` ・ `k8sc_*.py`）。

## 条件

- 版: foundation ef7306d と、その上の直し。release ＋ 開発の仕込み（`tools/measure/build-dev.sh`、`CARGO_TARGET_DIR=target/k8sc`）。前の版の `meloet` は同じ所から作り、`target/k8sc/before/release/` に写して、前 ・ 後を同じ時間帯に交互に動かした。
- WebKitGTK `2.52.6-0ubuntu0.26.04.2`、CPU の道（`crates/meloet-app` が `WEBKIT_DISABLE_COMPOSITING_MODE=1` を決める版）。窓 1920×1080。画面は 144 Hz（`xrandr`: DP-5 143.87 Hz）。
- **PC は静かでなかった**: ほかに 2 人の係（メモリ、Surge XT）が、ビルドや本物のアプリを動かしていた。回の始めの load（1 分）を表に書いた。前と後は交互なので、くらべは同じ条件。
- 曲: `arr16`（`target/measure/round8-quiet/songs/arr16.meloet` の写し。16 本、どのトラックも 200 小節のクリップ 1 つ、テイクのノートはトラックごとに 2000）と、歌の曲（`target/measure/round7-step3/song16/songs/はじめての曲` の写し、`--splits 11` で歌のクリップ 192 個）。
- 道具: `tools/measure/arrange_parts.py --how task --zooms 4`（出来事はコマのあとのタスクで 1 コマに 1 つ。`sideways` 240 回、いちばん縮めて横に送る `sidewaysOut` 120 回、拡大 ・ 縮小を 10 段ずつ 4 回 `zoomMore` 80 回）。

## 1. どこに時間を使っていたか（直す前）

- **よごれた四角**（gdb の下で、`webkit_damage.py` を `arrange_parts.py` に使うスクラッチ `k8sc-damage.sh`）: 送るコマは、どれも「囲む四角 1 つ」で、**220,128 から 1676×904（151.5 万 px）**。中身は、クリップの面の canvas（1676×836）、目盛の数字 20〜50 個（15×16 ずつ）、札の行 13 本（1675×16）、横の棒。目盛の数字が 1 つずつ別の四角になるので、四角は 36〜108 個（11 個をこえる）。けれど、送ると時間の側はぜんぶ変わるので、囲む四角は窓の時間の側と同じ広さで、四角をへらしても描く広さはへらない。
- **主スレッドのスタック**（`webkit-stacks.sh`、997 回）: 描く所（`DrawingAreaCoordinatedGraphics::display`）56 %。そのうち canvas を描く所 23 %（GPU からの読みもどし `readPixels` 2 回 12.7 %、写す 14 %）、地をぬる所 16 %（`paintRootBoxFillLayers` → `rect_memset32`。描くたびに新しく取る共有メモリの最初の書きこみ）、文字 7 %。スタイルの計算とレイアウトは 3〜4 %。のこりは JavaScript（JIT の中は名前が出ない）。
- **部品をかくしてくらべる**（`--hide`、前の版、1 回ずつ。主スレッドの CPU / コマ）: canvas をかくすと 12.5 → 9.1 ms、札 → 11.3〜12.8、目盛の数字 → 12.5、時間の側ぜんぶ（`.time{visibility:hidden}`）→ 8.8 ms。**時間の側を描かなくても 8.8 ms のこる**: ホイールの係の JS が 1 回 3 ms（`jsMs`）。
- **JS の中身**（`$derived` に時間をはかる仕込みを一時的に入れた版。のち外した）: ホイールの JS の 8 割がクリップの面の四角を作る所（`lanePicture`）。送るたびに、見えているクリップのテイクのノートをぜんぶ形にしていた（`arr16` は 200 小節のクリップなので、1 回に 32000 個）。
- **busyMs の読みかた**（gdb で `displayDidRefresh` ・ `display` ・ `Page::updateRendering` を数えた。`k8sc_count.py`）: CPU の道では、画面のプロセスの知らせ → 前のコマを描く → 次のコマ（rAF）がひと組で、コマは描き終えしだい次が来る（画面は 144 Hz なので 16.7 ms を待たない）。busyMs はコマの頭から、そのコマの絵を描き終えるまでで、知らせが次のコマの描く所のあとに回ると 2 つぶんになる（直したあとの分布は 9 ms と 18 ms の 2 つの山）。**60 Hz を保っているかは、コマの間（`frameGapMs`）で読む**。道具に平均と p95 を足した。

## 2. 前と後（`arr16`、3 回ずつ交互。load 0.95〜1.74）

数は 3 回の幅。busyMs は 平均 / p95、コマの間は 平均 / p95、CPU は WebKitWebProcess の主スレッドの 1 コマあたり。

| 送り | | busyMs | コマの間 | CPU | ホイールの JS |
|---|---|---|---|---|---|
| 横に送る（描いたコマ 200〜203） | 前 | 19.7〜20.2 / 23〜26 | 13.6〜13.9 / 15〜16 | 12.1〜12.4 | 2.9 |
| | 後 | **12.7〜14.1** / 21〜22 | **11.0〜11.2 / 12〜13** | **9.4** | 0.7 |
| いちばん縮めて横に送る（120） | 前 | 23.8〜24.7 / 30〜36 | 14.8〜15.2 / 18〜22 | 14.7〜15.2 | 4.5〜4.6 |
| | 後 | **14.4〜16.4** / 23〜29 | **10.5〜11.1 / 13〜16** | **10.5〜11.0** | 0.7〜0.8 |
| 拡大 ・ 縮小（80） | 前 | 20.3〜21.4 / 34〜35 | 12.4〜12.9 / 14〜17 | 12.4〜12.9 | 3.6〜3.7 |
| | 後 | 19.3〜19.7 / 35 | 11.2〜11.4 / 15〜16 | 11.2〜11.4 | 2.4 |

- 横に送るのは、busyMs の平均が 16.7 ms の中に入った。コマの間は平均 11 ms、p95 12〜16 ms（60 Hz より速い）。busyMs の p95（21〜29）は、上の「2 つぶん」の山。
- 拡大は、JS は 3.6 → 2.4 ms にへったが、busyMs の平均は 19〜20 ms のまま（目あてに届いていない）。コマの間は 11 ms。拡大は数字 ・ 札 ・ マーカー ・ テンポの字がぜんぶ動き、面もぜんぶ描くので、送りより重い。
- 1 回だけの途中の数: 後の版で canvas をかくすと（`--hide`）、横送り 13.3 → 8.5 ms、拡大 19.2 → 13.3 ms（`z-*`）。**のこりで一番大きいのは canvas を描く所**（WebKit の中。下の 4 章）。

## 3. 歌のクリップ 192 個（2 回ずつ交互。load 0.71〜1.15）

| | 前 | 後 |
|---|---|---|
| 横に送る（描いたコマ 116。曲が短く、はしで止まる） | 21.7〜22.0（CPU 12.3〜12.5） | 21.5〜21.6（12.2〜12.3） |
| いちばん縮めて横に送る（192 個ぜんぶ） | 31.6〜31.7（CPU 18.0〜18.1） | 31.2（17.6） |
| 拡大 ・ 縮小 | 21.6〜22.1（12.5〜12.7） | 22.5〜22.7（12.7〜12.8） |

- 変わらない（悪くもならない）。この曲の重さは、札 192 個の文字を描く所と波形で、作りなおしをやめた四角ではない（22-18 の「書体の `@font-face`」の宿題）。

## 4. ためして、とらなかった形

- **札を 2 組に分ける**（左はしより右の札は曲の頭からの x で置いて組ごと `transform`、左はしにかかる札は画面の x）: 歌の曲で CPU が 1 ms ほどへったが、`arr16` のいちばん縮めて送る所（曲の頭で左右に 40 px）で 14〜16 → 20 ms に悪くなった。札が左はしをまたぐたびに組をうつり、要素を作りなおすため。とらなかった。
- **クリップの面を `bitmaprenderer` の写しで見せる**（ミキサーの半分の形を 1 枚で）: 変わらなかった（busyMs 20.2〜20.7 と 20.9〜22.2）。写した絵も WebKit が同じように読みもどす、と見た（確かさ C）。とらなかった。
- **WebKit の 2D canvas の加速を切る**（`webkit2gtk` 2.0.2 の `SettingsExt::set_enable_accelerated_2d_canvas(false)`）: スタックでは、WebGL の絵を CPU に読み（`readPixelsForPaintResults`）、それを GPU の ImageBuffer（`ImageBufferSkiaAcceleratedBackend`）に入れ、描くときにまた読みもどしている（`getROPixels` → `GrGLGpu::onReadPixels`）。canvas の ImageBuffer が加速でなければ、読みもどしが 1 回になるかもしれない。`crates/meloet-app` の書きかえが要るので、ためしていない（棟梁へ）。

## 5. 再生中の鍵 → 画面（`latency.sh --no-build --state playing --jitter-ms 100`、2 回ずつ交互。load 1.82〜6.05）

| | p50 | p95 | 最大 | 2 コマ以内 | 鍵 → DAC p95 |
|---|---|---|---|---|---|
| 前 | 0.56 ・ 0.58 | 1.93 ・ 2.40 | 4.35 ・ 4.53 | 100 ・ 100 % | 15.65 ・ 15.84 |
| 後 | 0.75 ・ 0.60 | 3.42 ・ 2.54 | 4.78 ・ 7.38 | 100 ・ 100 % | 15.61 ・ 15.63 |

- 変わらない（静かな PC の CPU の道: p50 0.4〜0.6、p95 1.1〜3.4、最大 4.0〜5.1。`2026-11-22-round8-quiet.md`）。何もない画面なので、アレンジの面は動いていない。

## 6. 見た目

- `tools/design/shoot_ui.py`（headless Chrome、偽物の場面）で、前と後の絵を同じ場面で撮り、画素ごとにくらべた: `loop` ・ `takesArrange` ・ `audioEdit` ・ `audioTall` ・ `tempo` ・ `recording` ・ `vocalRec` ・ `arrangeZoomIn` の 1280×800 ・ 1920×1080、100 ・ 150 %（29 枚）。**29 枚とも 1 画素もちがわない**。どの場面も送っていない所（scrollX 0）なので、送ったあとの絵は、試験（`ui/tests/arrange.zoom.test.ts` の「横に送る」）で、画面の x が前と同じ式になる事を見た。本物のアプリで送ったあとの絵はくらべていない。

## 読んだ所

- `docs/architecture.md` 7 章 ・ 22-18、`docs/measurements/2026-11-22-round8-quiet.md` ・ `2026-11-22-round8-webkit-2.52.6-2.md`。
- WebKitGTK 2.52.6-0ubuntu0.26.04.2 の記号表（debug 情報の `nm -C`）: `DrawingAreaCoordinatedGraphics::{display,displayDidRefresh,setNeedsDisplayInRect}`、`WebCore::Page::updateRendering`、`WebPreferences::canvasUsesAcceleratedDrawing`。
- `webkit2gtk` 2.0.2 の `src/auto/settings.rs`（`set_enable_accelerated_2d_canvas`）。
- `ui/src/gl/{engine,batch,paint,slot}.ts`、`ui/src/components/{Arrange.svelte,arrangeGeom.ts}`、`ui/src/components/arrange/{wave,audioClipGeom}.ts`。
