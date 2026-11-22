# 第 8 回 区切り4: CPU の道で WebKitWebProcess のメモリはふえつづけるか（2026-11-22、meloet-hard ・ 印 k8wk）

Fable 5.2 の上限のため Opus 5.5 で動いた。決めた事は `docs/architecture.md` 23-15。

## 条件

- 版: foundation `91c0f43`（a- の回、`d-mmap`、`g-pas`）と `001ccc4`（k8sc のアレンジの送りを入れた物。そのほかの回）。release ＋ 開発の仕込み（`tools/measure/build-dev.sh`、`CARGO_TARGET_DIR=target/k8wk`）。窓 1920×1080、48000 Hz、quantum 256（変えていない）。
- WebKitGTK `libwebkit2gtk-4.1-0` 2.52.6-0ubuntu0.26.04.2、glibc 2.43。
- 道具: 新しく `tools/measure/webkit_memory.py`（`run` ・ `summary`）。30 秒ごとに、WebKitWebProcess と meloet の `smaps_rollup`、`/proc/<pid>/status`、種類ごとの RSS（`memory.py` の分け方）、fd の数と `memfd:` の数 ・ 大きさ、無名と共有の領域そのもの（`maps.jsonl`）。回ごとに別の `XDG_DATA_HOME` ・ `XDG_CONFIG_HOME` ・ `XDG_STATE_HOME`（絶対の道）。
- 場面: 止まっている ・ 再生（`plain-long`: `make_song.py --notes 12000 --bars 800`、2000 秒）・ ミキサーをひらいた再生（`band8-long`: `--tracks 8 --notes 12000 --bars 800 --lanes volume,pan,send:a,send:b --lane-points 3200`）・ 録る（新しい曲 ＝ グランドピアノ、仮想の鍵 1 秒 4 つ、単音 ・ 和音 ・ 速い打鍵まぜ）。
- 道: CPU の道 ＝ 今の既定（`main.rs` が決める `WEBKIT_DISABLE_COMPOSITING_MODE=1`）、合成の道 ＝ 外から `WEBKIT_DISABLE_COMPOSITING_MODE=0`（WebKit の `HardwareAccelerationManager` は「0 以外」で切る）。毎回 WebKitWebProcess のスレッドの名前（`ThreadedCompositor` があるか）で道を確かめた。
- まわり: **静かではない**（メモリの回）。同じ時に 3〜6 個の meloet（私の回）と、はじめの 10 分は別の係のアプリ。load 2〜9.7（各回の平均 3.8〜5.7）。オーナーさんの meloet は動いていなかった（`ps -u wintermaples`）。時間の回（6 章）は、ほかに何も動いていない時に取った（load 0.5〜1.0）。
- **画面が消えていた時間がある**: 19:45 ごろから、オーナーさんの画面が消えた（か窓が見えなくなった）らしく、**合成の道のページが 1 コマも描かなくなった**（ページの数えるコマ: `a-play-comp` は 30 分で 90771 ＝ 毎秒 51、`b-rec-comp` は 56063 ＝ 毎秒 32、`c-mix-comp` は 6710、20:24 からの時間の回の合成の道は 0。WebKit の `ThreadedCompositor` と `SkiaGPUWorker` の CPU も止まっていた）。CPU の道はその間も毎秒 62〜68 コマ描きつづけた。だから**合成の道の数は、描いていた時間（`a-play-comp` は 1500 秒まで、`b-rec-comp` は 900 秒まで）だけでくらべる**。合成の道のミキサーのメモリは取れていない。
- 生の数: `target/k8wk/wk/`（回ごとのフォルダ。`samples.jsonl`、`maps.jsonl`、`meta.json`、`raw/`）。スクラッチの道具は、係のスクラッチの `k8wk-*`（mmap を記録する LD_PRELOAD の `k8wk-mmaplog.c`、gdb で関数を数える `k8wk-count.py`、libpas の報告を読む `k8wk-passeries.py`）。

## 1. ふえかた（MB、WebKitWebProcess の RSS。2 分目から）

| 道 | 場面 | 2 分 → 25 分 | 30 分（2 分 → 29.5 分） | 最後の 3 分の 1 の傾き | 60 分 | 上限 |
|---|---|---|---|---|---|---|
| CPU | 止まっている | 410.9 → 411.8 | ＋1.0 | 0 | — | 平ら（30 分で描いたコマは 182） |
| 合成 | 止まっている | 456.6 → 463.0 | ＋6.4 | 0 | — | 平ら |
| CPU | 再生（`91c0f43`） | 367.2 → 385.0（＋17.8） | ＋17.5 | 0 | 367.2 → 405.3（＋38.1） | 段になってふえる（〜20 分 ＋8、〜40 分 ＋20）。段の間は平ら。60 分では止まりきっていない |
| 合成 | 再生（`91c0f43`） | 486.1 → 502.0（＋15.9） | （25 分目から描いていない） | — | — | 25 分までは CPU の道と同じくらい |
| CPU | ミキサーの再生 | 459.4 → 482.3（＋22.9） | ＋22.2 | ＋0.004 MB/秒 | — | 20〜25 分で平ら |
| 合成 | ミキサーの再生 | — | — | — | — | 取れていない（画面が消えていた） |
| CPU | 録る | 2 分 → 15 分: 442.6 → 462.1（＋19.5） | ＋28.5 | −0.014 MB/秒 | — | 20 分ごろから平ら |
| 合成 | 録る | 2 分 → 15 分: 527.4 → 544.3（＋16.9） | （15 分目から描いていない） | — | — | — |
| CPU | 10 分録る（`long-record.sh --seconds 600`、k8mem と同じ形） | 30 秒 → 600 秒: 353 → 380 | — | — | — | k8mem の回は 336 → 468 |

- **「CPU の道でふえつづける」は、今の版では出なかった**。CPU の道と合成の道は、同じ場面で同じくらいふえ（再生 25 分で ＋18 と ＋16、録る 13 分で ＋20 と ＋17）、どちらも 20〜25 分で平らになる（再生の CPU の道だけ、60 分で 2 段目が来た）。止まっていると、どちらの道も平ら（CPU の道は 30 分で 182 コマしか描かない）。
- **1 時間の見こみ**: CPU の道の再生で ＋40 MB ほど（測った数）。k8mem の見こみ（＋0.8 GB）は、起動してはじめの 2 分（ページの温まり。どの回も ＋15〜45 MB）を毎秒の傾きにした数だった、と見る（推論。確かさ C。入力は上の数と k8mem の表の「各 120 秒、起動から」）。ただし k8mem の 10 分録る（＋130）は、同じ道具 ・ 同じ形で今回 ＋27 だった。ちがいのわけは分からない（版のちがい `ef7306d` → `001ccc4`、まわりのアプリ）。
- **RSS の高さは、CPU の道のほうが低い**: 同じ場面の 2 分目で、再生 367 と 486、録る 443 と 527、ミキサー 459 と 636、止まっている 411 と 457。合成の道は `gpu`（NVIDIA の写し）と共有メモリ（`memfd` 2 つ、15.8 MB）と arena を多く持つ。
- 起動のあとの高さは、回ごとに 2 つの形に分かれる（`heap` 45 ＋ `arena` 22 ＋ `gpu` 66 MB と、`heap` 18〜27 ＋ `arena` 0 ＋ `gpu` 52〜57 MB。50〜70 MB の差）。ふえかたには関係しない。どちらになるかは決まっていない（道 ・ 版によらない）。

## 2. どこがふえるか

- **ふえるのは無名の領域（`anon`）だけ**。`heap`（`[heap]`）・ `arena` ・ `shm` ・ `gpu` ・ `file` は、どの回も 2 分目からほぼ同じ（± 1 MB）。
- **共有メモリはたまっていない**: CPU の道の WebKitWebProcess の fd は 30 分で 55 → 55、`memfd:` は 0〜2 個（描いている途中のもの）、`RssShmem` 21〜28 MB のまま。meloet（画面のプロセス）も、fd 93〜98、`memfd:` 8 個、RSS ± 4 MB（どの回も）。
  - CPU の道では、1 コマごとに `SharedMemory::allocate`（`memfd` を作って mmap）→ 描く → 画面のプロセスへ渡す → munmap（WebKitGTK 2.52.6 の `DrawingAreaCoordinatedGraphics::display(UpdateInfo&)` が `ShareableBitmap::create` を毎回呼ぶ。LD_PRELOAD で mmap ・ munmap を記録した `d-mmap` の回でも、`SharedMemory::allocate` の mmap は 3544 回、munmap と同じ数で、残らない）。
- **ふえる無名の領域の持ち主は libpas**（WebKit の `fastMalloc` と JavaScriptCore の入れ物）: mmap を呼んだ所ごとに RSS を足すと、30 秒でふえた ＋5.1 MB はすべて `pas_page_malloc_try_allocate_without_deallocating_padding` が取った領域だった（`d-mmap`）。glibc の malloc（`[heap]`・ arena）はふえない。
- libpas の報告（`pas_status_reporter_enabled = 2` を LD_PRELOAD の初めに書き、60 秒ごとに `pas_status_reporter_dump_everything`。`r-pas`、CPU の道で録る）: 使っているのは `bmalloc` の「Common Primitive」の入れ物（使用中 65.5 → 66.8 MB、確保 74.8 → 76.6 MB、1 分）。こまぎれ（Frag）は 12〜13 % で、ふえたのは「使用中」。だから**手放したのに返さない（malloc の動き）ではなく、ページの中で生きている物がふえている**（JS の値、DOM、WebKit の中の物。この報告では分けられない）。
- glibc の malloc の決まりをかえても、ふえかたは変わらない（4 章）。

## 3. WebKitWebProcess が消えた件

- **再現しなかった**。CPU の道で合わせて約 4 時間（30 分 × 3、60 分 × 1、10 分 × 6、ほかに 5〜15 分の回 3 つ）、どの回も WebKitWebProcess は 1 つのまま（`meta.json` の `webs`）、meloet の記録に `web process terminated` はない。
- 1 回だけ消えたのは、私が gdb で WebKit の中の関数（`pas_status_reporter_dump_everything`、heap の鍵を持たずに呼んだ）を呼んだ時で、SIGILL（libpas の assert）。meloet の見はり（`webprocess.rs`）はページを読みなおし、測りは 150 秒目から新しい pid で続いた。apport がその報告を `/var/crash/_usr_lib_x86_64-linux-gnu_webkit2gtk-4.1_WebKitWebProcess.1001.crash`（19:27:54、pid 1955325）に書いた。**この報告は私が起こした物**（消そうとしたが、私の権限の決まりで止められた。棟梁かオーナーさんが消してよい）。
- k8mem の回のわけは、今は分からない。apport がこの報告を書けた事から、k8mem の回の後に「まだ見ていない WebKitWebProcess の報告（ユーザー 1001）」はなかった。つまりあの回は、落ちて報告が残る形（SIGSEGV など）ではなかったかもしれない（確かさ C）。`dmesg` は読めない（権限）、`journalctl` に OOM の記録はない。

## 4. ためした事（CPU の道、録る、10 分、同じ時に 5 つ。2 分 → 10 分の RSS）

| 形 | 起動のいちばん初めに決めた物 | 2 分 → 10 分 | 最後の 3 分の 1 の傾き | 描いたコマ |
|---|---|---|---|---|
| 今のまま | — | 443.6 → 453.9（＋10.3） | ＋0.045 MB/秒 | 40551 |
| bmalloc を使わない | `Malloc=1`（WebKitWebProcess が受けつぐのを `/proc/<pid>/environ` で確かめた） | 385.9 → 414.9（＋29.0。`heap` ＋8、`arena` ＋12、`anon` ＋10） | ＋0.071 | 40589 |
| glibc の malloc の決まり | `GLIBC_TUNABLES=glibc.malloc.trim_threshold=131072:glibc.malloc.arena_max=2:glibc.malloc.mmap_threshold=131072`（同じく確かめた） | 369.4 → 379.6（＋10.2） | −0.03 | 40291 |
| 設定で CPU の道 | `WEBKIT_DISABLE_COMPOSITING_MODE=0` ＋ `Settings::set_hardware_acceleration_policy(Never)`（`with_webview` で） | 379.6 → 388.2（＋8.6） | ＋0.045 | 40458 |
| 2D の canvas を GPU にしない | 今のまま ＋ `enable-2d-canvas-acceleration` を false（`set_property`） | 449.2 → 461.1（＋11.9） | ＋0.03 | 40599 |

- **どれも、今のままと同じくらい**（10 分で ＋9〜12 MB。`Malloc=1` だけ悪い）。ふえる物は glibc の malloc でも libpas のこまぎれでもないので、malloc の決まりでは変わらない（2 章とそろう）。
- 設定で CPU の道（`Never`）: ページは CPU の道で描く（`ThreadedCompositor` がない）。ただし `Core: Scrolling` のスレッドが残る（WebView を作った時の設定は「いつも合成」で、あとから変えたため）。WebKitGTK 2.52.6 の `HardwareAccelerationManager` は、環境の変数で `canUseHardwareAcceleration` を落とし、設定の `Never` は `acceleratedCompositingEnabled` と `forceCompositingMode` を落とす。描く道は同じ。
- `webkit2gtk` 2.0.2 の `set_enable_accelerated_2d_canvas` は、WebKitGTK 2.52.6 では**何もしない**（`webkit_settings_set_enable_accelerated_2d_canvas` は引数を確かめるだけ）。生きているのは 2.46 からの `enable-2d-canvas-acceleration`（`canvasUsesAcceleratedDrawing`）で、`webkit2gtk` 2.0.2 には関数がないので `glib` の `set_property` で決めた。

## 5. canvas の読みもどし（棟梁の足した頼み）

- **`enable-2d-canvas-acceleration` を false にしても、WebGL の canvas の読みもどしは変わらない**。gdb で数えた（`band8-all` の再生、8 秒）: `ImageBufferSkiaAcceleratedBackend::create` は今のまま 956 回、false で 958 回（毎秒 120 回、canvas ごとに 1 コマ 1 回）、`createNativeImageReference` 1532 回と 1524 回。CPU の ImageBuffer（`ImageBufferSkiaUnacceleratedBackend`）は 0 回。
- わけ（WebKitGTK 2.52.6 のソース、`Source/WebCore/html/canvas/WebGLRenderingContextBase.cpp`。確かさ A）: CPU の道で WebGL の canvas を描くと `paint` → `surfaceBufferToImageBuffer` → `createImageBufferForWebGLContextReads` になり、これは `ImageBuffer::create(size, RenderingMode::Accelerated, …)` と**GPU の ImageBuffer を決めうちで作る**（設定を見ない）。読んだ物は `m_readDisplayBuffer` にとっておくが、`prepareForDisplay`（WebGL が新しいコマを描くたび）で捨てるので、毎コマ作りなおし、`copyNativeImageYFlipped`（GPU → CPU）→ `paintToCanvas`（GPU の ImageBuffer へ）→ ページの絵に描く時にまた読む。gdb の呼び出しの列（`k8wk-count.py`、debug 情報つき）: `ImageBufferSkiaAcceleratedBackend::create` ← `createImageBufferForWebGLContextReads` ← `surfaceBufferToImageBuffer` ← `WebGLRenderingContextBase::surfaceBufferToImageBuffer` ← `paint` ← `paintReplaced` ← ページの層を描く所。
- 時間（6 章）も変わらない。この形は、WebKit の側を変えないと 1 回にならない（ページの側でできる事は、canvas を出すコマを減らす事だけ）。

## 6. 時間（静かな PC、20:19〜20:38、load 0.5〜1.0。`arr16` と `band8-all` は `target/measure/round8-quiet/songs/` の物。2 回ずつ、交互）

| 形 | 拡大（`arrange_parts.py --how task --zooms 4` の `zoomMore`）busyMs 平均 / p95、コマの間 平均 / p95、主スレッド CPU / コマ | 横に送る busyMs 平均、コマの間 平均 | 鍵 → 描いた（`latency.sh --state playing --jitter-ms 100`）p50 / p95 / 最大、2 コマ以内 | ミキサーの再生（`app-watch.sh` 30 秒、10〜26 秒目）busyMs 平均 / p95、コマの間 p95、25 ms をこえた数 |
|---|---|---|---|---|
| 今のまま（CPU の道） | 17.8〜18.6 / 33〜34、10.7〜11.0 / 14、10.7〜10.9 ms | 11.3〜11.7、10.0〜10.5 | 0.58〜0.66 / 2.7〜3.3 / 4.7〜6.6、100 % | 9.80〜9.85 / 12、17、0〜1 |
| ＋ `enable-2d-canvas-acceleration` false | 18.1〜18.2 / 33、10.6〜10.8 / 13〜14、10.6〜10.7 | 11.4〜12.3、10.8〜11.0 | 0.54〜0.63 / 2.2〜3.3 / 5.9〜7.4、100 % | 9.78〜9.88 / 11〜12、17、0 |
| 設定で CPU の道（`Never`） | 18.5〜19.2 / 33〜34、10.9〜11.2 / 14〜15、10.9〜11.2 | 12.0〜12.7、10.8〜11.0 | 0.40〜0.45 / 1.9〜2.8 / 3.6〜6.3、100 % | 9.74〜9.77 / 12、17、0〜1 |
| 合成の道 | 取れない（ページが描かない。条件の「画面が消えていた」） | — | 取れない（120 鍵とも描かれなかった） | 取れない |

- 3 つの CPU の道の形は、2 回の幅の中で同じ。合成の道の数は `2026-11-22-round8-quiet.md`（鍵 → 描いた p50 7.6〜10.0 ms、ミキサーの busyMs 12.4〜12.5 ms）と、`2026-11-22-round8-webkit-2.52.6-2.md`。
- ミキサーの RSS（30 秒）: meloet 911〜916 MB、WebKitWebProcess 425〜427 MB（3 つの形とも同じ）。

## 7. 気づいた事（頼まれていない）

- **CPU の道は、画面が消えても描きつづける**。合成の道は、画面が消えると（Wayland の frame callback が来ないので）描かなくなるが、CPU の道は前のコマを画面のプロセスへ渡し終えしだい次を描くので、再生中は毎秒 60 コマ以上を描きつづけた（`a-play-cpu` は 60 分で 223068 コマ ＝ 毎秒 62）。窓をかくした時や画面が消えた時に、WebKit の CPU（再生中 20 % ほど、ミキサーで 70 % ほど）を使いつづける。止まっている時はほとんど描かない（30 分で 182 コマ）。

## 読んだ所

- WebKitGTK 2.52.6 のソース（GitHub の WebKit のタグ `webkitgtk-2.52.6`）: `Source/WebKit/WebProcess/WebPage/CoordinatedGraphics/DrawingAreaCoordinatedGraphics.cpp`（`display(UpdateInfo&)`）、`Source/WebCore/platform/graphics/ShareableBitmap.cpp`、`Source/WebKit/UIProcess/gtk/HardwareAccelerationManager.cpp`、`Source/WebKit/UIProcess/API/glib/WebKitSettings.cpp`（`set_hardware_acceleration_policy`、`set_enable_accelerated_2d_canvas`、`set_enable_2d_canvas_acceleration`）、`Source/WebCore/html/canvas/WebGLRenderingContextBase.cpp`（`createImageBufferForWebGLContextReads`、`surfaceBufferToImageBuffer`、`prepareForDisplay`）、`Source/bmalloc/libpas/src/libpas/pas_status_reporter.c` ・ `pas_scavenger.c`。この PC の 2.52.6-0ubuntu0.26.04.2 の debug 情報（`~/.cache/debuginfod_client/e94c7901…`）で、呼び出しの列と関数の場所を確かめた。
- `~/.cargo/registry/src/*/webkit2gtk-2.0.2/src/auto/settings.rs`（`set_hardware_acceleration_policy`、`set_enable_accelerated_2d_canvas`。`enable-2d-canvas-acceleration` の関数はない）、`webkit2gtk-sys-2.0.2`、`wry-0.57.2/src/webkitgtk/mod.rs`（WebView の設定は wry が作る時に決める。あとから `with_webview` の `inner()` でさわれる）。
