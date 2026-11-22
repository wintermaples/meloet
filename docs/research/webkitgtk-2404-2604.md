# Ubuntu 24.04 と 26.04 の WebKitGTK

2026-11-22　調べ物の係（meloet-research）

## 1. 結論（短く）

1. **版は同じになった。** 24.04 も 26.04 も、いま `libwebkit2gtk-4.1-0` は **2.52.6**（24.04 は `2.52.6-0ubuntu0.24.04.2`、26.04 は `2.52.6-0ubuntu0.26.04.2`）。ソースが同じなので、「11 個 ・ 25 %」の決まり、`JSC_useGenerationalGC`、WebGL の 16 個は、**24.04 でも同じ**。【確かさ A】
2. **いちばん大事な発見: この PC の `libwebkit2gtk-4.1-0` は、2026-11-21 23:41 に `.1` から `.2` に上がっていた**（`/var/log/dpkg.log` の 2749 行目）。`.2` の中身は「NVIDIA では SHM の運びかたを使う」という直し。meloet のこれまでの測定（2026-11-15〜11-21）と docs の「CPU で描く WebKit」は、ぜんぶ **`.1` の上で取った数**。`.2` で描きかたが変わった見こみがある。【事実は A。「変わった見こみ」は私の推論で C】
3. 24.04 と 26.04 の本当のちがいは、WebKit の中ではなく、まわりの部品（GTK、Mesa、コンパイラ、Wayland の合成器）。これで描きかたが変わるかは、調べきれなかった。【C】

## 2. くわしい中身

### 2-1. 版（いま 2026 年 11 月）

| | 出たときの版 | 更新で届いた版 |
|---|---|---|
| 24.04 (noble) | 2.44.0-2（2024 年春。Debian から） | -security: 2.46.1（2024-10-08）→ 2.48.x → 2.50.x → 2.52.3（2026-04-23）→ **2.52.6-0ubuntu0.24.04.1（2026-08-21、USN-8703-1、2026-08-31）** → **.2（changelog 2026-09-28）** |
| 26.04 (resolute) | 2.52.0-1 | 2.52.3 → **2.52.6-0ubuntu0.26.04.1（2026-08-21）** → **.2（changelog 2026-10-23）** |

- つまり Ubuntu は、24.04 にも WebKitGTK の新しい枝（2.46 → 2.52）を、セキュリティ更新として入れつづけている。「24.04 は古い 2.44」ではない。【A: Ubuntu の changelog、Launchpad】
- `.2` は、2 つとも 2026-11-17 に -updates へ出た（Launchpad のバグ 2168736 の書きこみ）。【A】
- 24.04 の 2.44.x の更新が -security にあったかは、資料が食いちがう（changelog の要約では「なし」、メーリングリストでは 2.44.2 が 2024-05 にある）。いまの版には関係しない。
- 24.04 の周りの部品: GTK 3.24.41、Mesa 25.2.8（backport）。26.04: GTK 3.24.52、Mesa 26.0.x。【A: Launchpad】

### 2-2. 「CPU で描く」の正体（1 の問いの答え）

**決まりのコード**（WebKit の枝 `webkitglib/2.52`、`DrawingAreaCoordinatedGraphics.cpp`）【A】
- `shouldPaintBoundsRect`: `rectThreshold = 10`、`wastedSpaceThreshold = 0.75`。四角が 1 個か 10 個より多い（11 個以上）なら囲む四角を描く。そうでなければ「無駄 ≦ 75 %」（つまり四角の合計が囲む四角の 25 % 以上）のとき囲む四角を描く。meloet の読みと合っている。
- このコードは `Damage` クラスを使っていない。`ENABLE(DAMAGE_TRACKING)` は試験用の道具にしか出てこない。
- 2.44 の枝も同じ数（10 と 0.75）。つまり少なくとも 2.44 → 2.52 で変わっていない。【確かさ B。2.44 の枝は 2 年より古い】
- 24.04 と 26.04 は同じ 2.52.6 なので、この決まりは同じ。

**どちらの道（CPU か合成か）を使うか**（`HardwareAccelerationManager.cpp`、`AcceleratedBackingStore.cpp`、`WebKitSettings.cpp`、2.52 の枝）【A】
- `WebKitSettings` の `hardware-acceleration-policy` の既定は ALWAYS。ALWAYS を決めるとき、`canUseHardwareAcceleration()` が false なら**何もしない**（合成の層を使わない = CPU で描く道のまま）。wry 0.57.2 には、この設定や `WEBKIT_*` の変数を決める所がない（`~/.cargo/registry` を grep）。
- `canUseHardwareAcceleration()` が false になる 3 つ: WebGL を切ったビルド、`WEBKIT_DISABLE_COMPOSITING_MODE` が "0" 以外、`AcceleratedBackingStore::checkRequirements()` が false。
- `checkRequirements()` は、運びかたの集まり（`rendererBufferTransportMode()`）が**空**なら false。集まりが空になるのは、`WEBKIT_DISABLE_DMABUF_RENDERER` が "0" 以外のとき、EGL に GBM か surfaceless の拡張がないとき、など。
- `WEBKIT_FORCE_COMPOSITING_MODE` は、"0" のときだけ意味がある（`m_forceHardwareAcceleration` を false にする）。`=1` は何も変えない。**meloet が見た「`WEBKIT_FORCE_COMPOSITING_MODE=1` でも変わらない」と合う。**

**NVIDIA での Debian / Ubuntu のパッチ**
- Debian は 2023-10（2.42.1-2）から、NVIDIA の独自ドライバーでは DMABuf の描画を切るパッチ（`disable-dmabuf-nvidia.patch`、のちに `disable-nvidia-dmabuf.patch`）を持っている。パッチは `isNVIDIA()`（GL の vendor 文字に "NVIDIA" があるか）で見つけ、`WEBKIT_FORCE_DMABUF_RENDERER` で上書きできる。Ubuntu の 24.04 にも 2.46.6（2025-02）から入っている。【A: Debian の changelog、Ubuntu の changelog、Launchpad のパッチ本文】
- Launchpad の `.1` 世代のパッチでは、`isNVIDIA()` の検査が `mode.add(SharedMemory)` より**前**にある。NVIDIA では運びかたの集まりが空になる。【A: Launchpad の git。パッチ本文】
- Debian のバグ #1142771 とそれの Ubuntu 版（LP #2168736）: この並びのせいで、NVIDIA で「合成の層がいるようになった最初の時」に SIGSEGV（`create()` が null を返し、呼ぶ側が確かめない）。直しは「NVIDIA でも SHM の運びかたは足す」（Debian 2.53.90-1、Ubuntu の `.2`）。【A: バグの書きこみと changelog。`.2` のパッチ本文そのものは見られなかった】

**ここからの私の読み**【確かさ C。A のコードをつなげた推論】
- `.1` では、NVIDIA の集まりが空 → `checkRequirements()` が false → `canUseHardwareAcceleration()` が false → ALWAYS が効かず、**CPU で描く道**（`display()` → ビットマップを UI のプロセスへ）になる。これが meloet が gdb で見た姿、という説明がつく。
- `.2` では、集まりが {SharedMemory} になる。GTK が GL の文脈を作れれば `canUse…` は true になり、**合成の層を使う道（Skia の層 → SHM のバッファ）**になるはず。meloet の「CPU で描く WebKit」の決まりが、そのまま効かなくなるかもしれない。ただし、実際にそうなったかは、この PC で見ていない（私は動かせない）。
- バグの書きこみに、検証した人が「epiphany だけでは `.2` の前の落ちが再現しなかった」と書いている（2026-11-10）。`.1` の NVIDIA でも、ふつうのページは落ちずに CPU の道で動くことと合う。

### 2-3. 24.04 の描きかた（Cairo か Skia か）

- **2.46.0（2024-09-17）で、2D の描画は Cairo から Skia に替わり、GPU で描くのが既定になった**。Cairo は、Skia がまだ動かない big-endian でだけ残る。【確かさ B: webkitgtk.org の公式の知らせ。2 年より古いので 1 段下げた】
- 24.04 に 2.46 が届いたのは 2024-10-08（2.46.1）。いまの 2.52.6 はずっと Skia。24.04 でも Cairo の道はない。
- 2.44（24.04 の出た時の版）の `AcceleratedBackingStore` は、DMABuf しか知らず SHM の道がない（2.44 の枝のコード）。【B】
- 2.50.0 の知らせに「UI プロセスへの damage の伝え（damage propagation）を既定でオン」とある。これは合成の層の道（`Damage` クラス）の話で、上の CPU の道の `shouldPaintBoundsRect` にはつながらない（コードを見た範囲）。【知らせは A。つながらないというのは C】
- 2.52.0 の知らせに、タイルの大きさが GPU の有無で変わる、動的 MSAA、スクロールのスレッドの改良がある。これらも合成の層の道の話。【A】

### 2-4. `JSC_useGenerationalGC`（2 の問い）

- 2.44 の枝にも 2.52 の枝にも、`OptionsList.h` に `v(Bool, useGenerationalGC, true, Normal, nullptr)` がある（別名 `alwaysDoFullCollection` は逆の意味）。
- `Heap::shouldDoFullCollection()` の中身も、どちらも同じ。`if (!useGenerationalGC()) return true;` が最初にある。意味は同じ。24.04 の 2.52.6 でも、`JSC_useGenerationalGC=0` は効くはず。【A】
- ただし「ひまなときに Eden の GC しか回らずメモリがふえる」ことが 24.04 でも起きるかは、同じソースなので起きるはずだが、24.04 では測っていない。【推論 C】

### 2-5. WebGL の文脈の数（3 の問い）

- 2.44 の枝も 2.52 の枝も、`WebGLRenderingContextBase.cpp` に `maxActiveContexts = 16`（主スレッド）と `maxActiveWorkerContexts = 4`（worker）。`addActiveContext` で、いっぱいになると、いちばん古く使われた文脈を `recycleContext()` で捨てる。`maxActiveWebGLContexts` という名前はない。
- 24.04 も 2.52.6 なので 16。【A（2.44 の枝は B）】

### 2-6. ほかの変わり（4 の問い）

| 版 | 変わり | 確かさ |
|---|---|---|
| 2.44.0（2024-03） | X11 と WPE の描画をやめ、DMA-BUF にまとめた。vblank の合わせかたを直した | B |
| 2.46.0（2024-09） | Skia が既定、GPU で描くのが既定。`enable-2d-canvas-acceleration` の設定ができた | B |
| 2.48.0（2025-03） | GPU で描くとき、タイルを worker スレッドで描く。WebGL の GPU プロセスのビルド（既定では使わない） | A |
| 2.50.0（2025-09） | 層を 1 回記録して worker で再生。UI プロセスへの damage の伝えが既定オン | A |
| 2.52.0（2026-03） | 動的 MSAA、タイルの大きさ、スクロールの改良 | A |

- `WEBKIT_DISABLE_DMABUF_RENDERER`: 2.52 では、**運びかたの集まりを空にする**ので、結果として `canUseHardwareAcceleration()` が false になり CPU の道になる。`WEBKIT_DISABLE_COMPOSITING_MODE`（"0" 以外）も、同じく CPU の道。【A】
- `requestAnimationFrame` の 60 Hz は、この PC では DRM の vblank が ENOTSUP で、固定 60 Hz のタイマーになる（`docs/notes-interim.md` の 3-1。meloet 自身の調べ）。24.04 で変わるかは調べていない。

## 3. meloet がたしかめる事

1. **今すぐ、この PC で。** `.2` に上がったあと（2026-11-21 23:41 以後）で、まだ CPU で描いているか。`webkit-damage.sh` で四角の並びを取る（`.1` のときは `shouldPaintBoundsRect` が gdb に出た）。`LayerTreeHost` や `ThreadedCompositor` の名前が出れば合成の層の道になっている。変わったなら、docs/architecture.md 7 章の「CPU で描く WebKit」と、区画 ・ `cover` ・ 25 % の決まりが、いつの版の話かを書く。
2. **道を固定するか決める。** CPU の道を前提にするなら、起動のいちばん初めに `WEBKIT_DISABLE_COMPOSITING_MODE=1`（`JSC_useGenerationalGC=0` と同じ所）を決めれば、どの版 ・ どの PC でも CPU の道に決まる（2.52 のコード。この PC で動かしてはいない）。外から同じ変数を決めた時にどちらが勝つかも書く。逆に、合成の層の道のほうが速いなら、その測定を取る。どちらが meloet に合うかは測ってから。
3. **24.04 で、同じことを見る**（コンテナでは GPU も画面もないので **VM か実機**。NVIDIA の独自ドライバーで）。
   - `dpkg -l libwebkit2gtk-4.1-0` が 2.52.6-0ubuntu0.24.04.2 か（`.1` なら NVIDIA で CPU の道、`.2` なら合成の層の道のはず）。
   - X11 と Wayland の両方。24.04 は GTK 3.24.41、26.04 は 3.24.52 で、GTK が GL の文脈を作れるか（`gtkCanUseHardwareAcceleration()` の結果）が変わるかもしれない。【C】
   - NVIDIA でない GPU（AMD / Intel の Mesa）では、DMABuf のハードウェアの道になるはず。meloet の速さの数は、**NVIDIA（SHM か CPU）と Mesa（DMABuf）で別に取る**必要がある。
   - `JSC_logGC=1` で GC の様子、10 分の録音（`long-record.sh`）で WebKitWebProcess のメモリ。
   - WebGL の文脈が 16 を超えないか（区画の数は最大 10、予算 12。同じコードなので変わらないはず）。
4. USN の `.2` のパッチ本文（`.2` の `debian.tar.xz`）を読んで、本当に「SHM を足した」のか確かめる。

## 4. 出どころの一覧（見た日はすべて 2026-11-22）

| URL | 書いてあったこと | 確かさ |
|---|---|---|
| https://launchpad.net/ubuntu/noble/+source/webkit2gtk | noble: release 2.44.0-2、security 2.52.6-…24.04.1、updates 2.52.6-…24.04.2 | A |
| https://launchpad.net/ubuntu/resolute/+source/webkit2gtk | resolute: release 2.52.0-1、security …26.04.1、updates …26.04.2 | A |
| https://changelogs.ubuntu.com/changelogs/pool/main/w/webkit2gtk/webkit2gtk_2.52.6-0ubuntu0.24.04.2/changelog | noble の changelog。2.46.1 から 2.52.6 までの日付、NVIDIA のパッチの歴史、`.2` の SHM | A |
| https://changelogs.ubuntu.com/changelogs/pool/main/w/webkit2gtk/webkit2gtk_2.52.6-0ubuntu0.26.04.2/changelog | resolute の changelog。`.2` は 2026-10-23 | A |
| https://ubuntu.com/security/notices/USN-8703-1 | 24.04 と 26.04 に 2.52.6（…24.04.1 と …26.04.1）。2026-08-31 | A |
| https://bugs.launchpad.net/ubuntu/+source/webkit2gtk/+bug/2168736 | NVIDIA で合成の層がいるとき SIGSEGV。`.2` が 2026-11-17 に -updates へ。検証の日付 | A |
| https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=1142771 | 原因（`isNVIDIA()` が SharedMemory より前）、直し（2.53.90-1 の SHM） | A |
| https://sources.debian.org/data/main/w/webkit2gtk/2.52.6-1/debian/changelog | NVIDIA のパッチの出入りの歴史（2.42.1-2、2.50.0-1 で外す、2.50.0-2 で戻す） | A |
| https://git.launchpad.net/ubuntu/+source/webkit2gtk/plain/debian/patches/disable-nvidia-dmabuf.patch?h=ubuntu/resolute-updates | `.1` 世代のパッチ。`isNVIDIA()` が SharedMemory の前 | A |
| https://mirrors.mit.edu/gentoo-portage/net-libs/webkit-gtk/files/2.52.3-disable-nvidia-dmabuf.patch | 同じパッチの 2.52.3 版（Gentoo の写し） | B |
| https://raw.githubusercontent.com/WebKit/WebKit/webkitglib/2.52/Source/WebKit/WebProcess/WebPage/CoordinatedGraphics/DrawingAreaCoordinatedGraphics.cpp | `shouldPaintBoundsRect`（10 と 0.75）、`m_alwaysUseCompositing` | A |
| https://raw.githubusercontent.com/WebKit/WebKit/webkitglib/2.44/Source/WebKit/WebProcess/WebPage/CoordinatedGraphics/DrawingAreaCoordinatedGraphics.cpp | 2.44 も同じ数 | B（2 年より古い） |
| https://raw.githubusercontent.com/WebKit/WebKit/webkitglib/2.52/Source/WebKit/UIProcess/gtk/HardwareAccelerationManager.cpp | 3 つの false の条件、`WEBKIT_FORCE_COMPOSITING_MODE` は "0" だけ | A |
| https://raw.githubusercontent.com/WebKit/WebKit/webkitglib/2.52/Source/WebKit/UIProcess/gtk/AcceleratedBackingStore.cpp | 運びかたの決めかた、`checkRequirements`、環境変数 | A |
| https://raw.githubusercontent.com/WebKit/WebKit/webkitglib/2.52/Source/WebKit/UIProcess/API/glib/WebKitSettings.cpp | `hardware-acceleration-policy` の既定 ALWAYS と、`canUse…` が false のときの動き | A |
| https://raw.githubusercontent.com/WebKit/WebKit/webkitglib/2.44/Source/WebKit/UIProcess/gtk/AcceleratedBackingStore.cpp | 2.44 に SHM の道がない | B |
| https://raw.githubusercontent.com/WebKit/WebKit/webkitglib/2.44/Source/JavaScriptCore/runtime/OptionsList.h と webkitglib/2.52 の同じファイル | `useGenerationalGC` が両方にある | A（2.44 は B） |
| https://raw.githubusercontent.com/WebKit/WebKit/webkitglib/2.44/Source/JavaScriptCore/heap/Heap.cpp と webkitglib/2.52 の同じファイル | `shouldDoFullCollection` の最初の枝が同じ | A（2.44 は B） |
| https://raw.githubusercontent.com/WebKit/WebKit/webkitglib/2.44/Source/WebCore/html/canvas/WebGLRenderingContextBase.cpp と webkitglib/2.52 の同じファイル | `maxActiveContexts = 16`、worker は 4 | A（2.44 は B） |
| https://webkitgtk.org/2024/03/16/webkitgtk2.44.0-released.html、…/2024/09/17/webkitgtk2.46.0-released.html、…/2025/03/14/webkitgtk2.48.0-released.html、…/2025/09/17/webkitgtk2.50.0-released.html、…/2026/03/18/webkitgtk2.52.0-released.html | 各版の知らせ（Skia、DMA-BUF、damage の伝え、ほか） | A（2.44 と 2.46 は日付が 2 年より前で B） |
| https://docs.webkit.org/Ports/WebKitGTK%20and%20WPE%20WebKit/Graphics.html | 非合成の道（SHM + UpdateInfo）と合成の道の説明 | A |
| https://launchpad.net/ubuntu/noble/+source/gtk+3.0、…/resolute/+source/gtk+3.0、…/noble/+source/mesa、…/resolute/+source/mesa | GTK と Mesa の版 | A |
| `/var/log/dpkg.log`（この PC） | libwebkit2gtk-4.1-0 が 2026-11-21 23:41 に .1 → .2 | A（この PC の記録） |
| `/srv/meloet-agent/meloet/docs/notes-interim.md`、`docs/report.md`、`docs/measurements/2026-11-19-round4-mixer.md`、`docs/architecture.md` 7 章 | meloet 側のこれまでの調べと測定（`.1` 世代の数） | meloet 自身の記録 |

## 5. わからなかったこと

- `.2` のパッチ本文そのもの（Launchpad の git の枝は `.1` 世代のままだった）。
- `.2` のあと、この PC が本当に合成の層の道に変わったか（私は動かせない）。
- 24.04 実機での見えかた（GTK 3.24.41 で GL の文脈が作れるか、X11 と Wayland のちがい）。
- 2.50 の damage の伝えが、CPU の道のほうの再描画に関わるか（コードを深く追っていない）。
- 24.04 の -security に 2.44.x があったか（資料が食いちがい）。
