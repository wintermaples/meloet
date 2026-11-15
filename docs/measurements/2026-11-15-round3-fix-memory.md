# 10 分録ると WebKitWebProcess がふえつづける: 分けた結果（第 3 回の直し、2026-11-15）

**直っていない。** 原因の形（JavaScriptCore の Full GC が走らない）まではわかった。JS の側で減らせる物は減らしたが、ふえる形は残った。

- 版: `4f056da` の上の作業ツリー。release、窓 1920×1080、バッファ 256、48000 Hz。
- PC: **静かではない**（別の係のビルド。load average 0.6〜6.2。各回の始めの値は下）。
- 道具: `tools/measure/long-record.sh --no-build [--seconds N] [--piano-roll]` に、環境変数 `JSC_logGC=1`（WebKitGTK の release でも効く。WebKitWebProcess が GC ごとに、前と後のヒープの大きさと Eden / Full を stderr に書く。`meloet.stderr`）。分ける形は、ビルドのときだけの仮のしかけ（`VITE_MELOET_EXP`。もう消した）で作った。
- 生の数: `target/r3-fix/m/v-*/`（2 分）、`target/r3-fix/m/long-{no-pr,pr}/`（10 分）。

## a. 分けた測定（ピアノロールなし。v-play だけ再生、ほかは録音）

| 形 | 秒 | Eden の回数 | Full の回数（前 → 後 MB） | Eden の後のヒープ MB（はじめ → 終わり） | WebKitWebProcess MB（20 秒 → 140 秒） |
|---|---|---|---|---|---|
| そのまま | 120 | 1503 | 0 | 16 → 85 | 357 → 456 |
| 録っているクリップの先（recordingClip）を描かない | 120 | 401 | 10（13→16、21→17、22→18、24→19 …） | 10 → 19 | 349 → 368 |
| 計器を JS で受けない（線も録っている先も止まる） | 120 | 28 | 1（13→16） | 10 → 45 | 352 → 418 |
| 上の 2 つとも | 120 | 3 | 11（21→17 …） | 10 → 16 | 336 → 351 |
| 直したあと（下の b） | 120 | 1714 | 0 | 16 → 66 | 357 → 442 |
| 直したあと、ピアノロールをひらいたまま | 120 | 1552 | 0 | 10 → 82 | 375 → 472 |
| 先の形は作るが、slot に渡さない | 90 | 1119 | 1（13→16） | 16 → 61 | 362 → 417（100 秒） |
| 先の四角を作らない（上から 2 つめと同じ所で戻る） | 90 | 1195 / 1264（2 回） | 1 / 0 | 10〜16 → 61 | 352 → 407（100 秒） |
| 同じ、入力計を前の形にもどして | 90 | 1172 | 1（13→16） | 16 → 72 | — |
| **録音せず、再生だけ** | 120 | 1614 | 0 | 16 → 59 | 340 → 415 |

- 「録っているクリップの先を描かない」と「先の四角を作らない」は、コードの上ではほぼ同じ所で戻る形だが、結果が大きくちがった（401 回と Full 10 回、対 1200 回と Full 0〜1 回）。3 回くりかえして後者の形だった。前者の 1 回は、たまたま Full が回りはじめた回と読む（確かさ C）。
- **再生だけ（録音しない）でも、同じ速さでふえる。** 録音にだけある物（録っているノート、録っているクリップの先）が原因ではない。
- どの「ふえる」回も、Full が 0 回か、はじめに 1 回（13 → 16 MB、つまり 1 つも返せなかった）だけ。Full が何度も回った 2 つの回だけが平ら。

## b. 直した事（効きは小さかった）

- `LiveNotesBook.notesOf` は、録っているノートが変わるまで同じ配列を返す（前は呼ぶたびに全部を並べなおした）。
- アレンジの録っているクリップの先は、毎コマ周のノート全部から形を作らず、先にかかるノートだけ形にする。周のノートと鍵の範囲は、ノートが変わったときだけ作る。
- 入力計は、60 Hz の計器で押している鍵が同じなら `$state` に新しい配列を入れない。
- Chrome（V8）で偽物の場面（`?scene=recordingMoving`）の割り当てを関数ごとに数えると、全部で 1 秒に約 0.9 MB（Svelte の更新 約 0.25 MB、メーターの `dbToLength` 約 0.13 MB、計器を受ける所 約 0.07 MB、再生まわりの時計の文 約 0.06 MB …）。どれも 60 Hz の仕事そのもので、ひとつで大半という物はない（確かさ B: V8 の数で、JSC の数ではない）。

## c. 直したあとの 10 分（確かめ）

| | ピアノロールなし | ひらいたまま |
|---|---|---|
| 始めの load average | 4.28 | 0.56 |
| テイクの鍵 | 2106 | 2106 |
| xrun（録音中） | 0（3 秒目、録音の前に 1） | 0（1 秒目に 1） |
| Eden / Full | 7874 / **0** | 7173 / **0** |
| Eden の後のヒープ MB | 16 → 276 | 16 → 312 |
| WebKitWebProcess MB（はじめ → いちばん大きい → 終わり） | 337 → 779 → 779 | 348 → 839 → 764 |
| 第 3 回（`2026-11-15-round3-long-record.md`） | 338 → 805 | 350 → 964 / 963 |

60 秒ごと（MB）:

| 秒 | 60 | 120 | 180 | 240 | 300 | 360 | 420 | 480 | 540 | 600 |
|---|---|---|---|---|---|---|---|---|---|---|
| なし | 383 | 419 | 463 | 503 | 543 | 581 | 620 | 660 | 710 | 752 |
| ひらいたまま | 406 | 451 | 507 | 556 | 597 | 641 | 682 | 728 | 781 | 826 |

- ふえる速さは、なしで約 0.7 MB/秒のまま、ひらいたままで約 1.0 → 0.8 MB/秒（計算: (826 − 406) ÷ 540 秒）。**ふえつづける形は変わらない。**

## d. 見立て

- 確かさ A（この PC の GC のログ）: ふえている間、JavaScriptCore は Eden の GC を 1 秒に 12〜14 回まわし、Full の GC を 10 分間 1 回もまわさない。Eden は、そのとき生きている物を古い側に上げ、古い側は Full でしか返さないので、60 Hz の仕事が一瞬だけ持っていた物（計器、四角の配列、Svelte の値など）が古い側にたまりつづける（JS のヒープが 16 → 276〜312 MB。RSS のふえ方の半分ほど。残りは、GC の物が持っている WebKit の中のメモリと読む）。
- 確かさ A（WebKit の 2.52 の枝のソース。調べ物の係が読んだ。行の照合はしていない）と C（当てはめ）:
  - WebKitGTK では、WebCore が JSC の GC のタイマーを「ひまな時の GC」（`OpportunisticTaskScheduler`）に差しかえている。描画のあとのひまな時に、割り当てが少しでもあれば Eden を走らせる（`VM::performOpportunisticallyScheduledTasks`）。1 秒に十数回の Eden は、これと形が合う。
  - Full は、(1) Full のタイマー（遅れは「前の Full の長さ ÷（ふえた量 × 前の Full で返せた割合 × 係数）」。前の Full で 1 つも返せなかったと、遅れが無限大になる）、(2) ひまな時の Full（旗が立つのは、ページの中のフレームの場所が変わるときだけ）、(3) メモリが 80〜90 % 使われたとき、のどれか。1 ページのまま動く meloet では、(2) と (3) はまず起きない。はじめの Full が「13 → 16 MB」（返せた割合 0）だった回は、そのあと Full が来ない。上の a と合う。
- だから、JS の割り当てを減らしても、Full が来ない限り、ゆっくりにはなっても止まらない。ページから正当に Full を起こす API は見つからなかった（`window.gc` なし、webkit2gtk-4.1 の公開 API にも GC を頼む関数はない（ヘッダを grep））。

## e. 棟梁に決めてほしい事（案）

1. そのままにして、上限を見る: 10 分で 750〜840 MB。1 時間なら 3 GB をこえる見こみ（計算: 0.7〜0.8 MB/秒 × 3600）。受けいれられない。
2. 画面の中の隠した iframe の場所を、ひまな時（録音・再生を止めたあと、など）に変えて、(2) の旗を立てる。仕組みの副作用を使う手で、2.52 で本当に Full が来るかは未確認。試すなら次の係で。
3. Rust 側: WebKit のプロセスに GC を頼む道（web extension で `jsc` の GC を呼ぶ、など）。meloet-app に手を入れることになる。未調査。
4. 60 Hz の仕事を、さらに大きく減らす（計器を JS で受ける所、Svelte の更新、メーター）。Full が来ない限り止まらないので、2 か 3 と組みあわせる物。

## f. memory pressure settings（WebKitGTK の `MemoryPressureSettings`、2026-11-15 夜）

**入れなかった。** ピアノロールなしでは RSS が平らになったが、ピアノロールをひらいたままでは効かなかった（下の表）。効くかどうかが起動の直後の時の運びで決まり、しくみも説明しきれていない。

### API の意味（WebKitGTK 2.52 の枝 `webkitglib/2.52` のソースを読んだ。入れた版は 2.52.6）

- `webkit_website_data_manager_set_memory_pressure_settings` は**ネットワークのプロセスだけ**に効く（その関数の文書とコード: `WebProcessPool::setNetworkProcessMemoryPressureHandlerConfiguration`）。ウェブのプロセスに効くのは `WebKitWebContext` の construct-only の property `memory-pressure-settings` だけで、wry 0.57.2（`src/webkitgtk/web_context.rs` の `WebContextImpl::new`）は `WebContext::builder()` にそれを渡さず、tauri-runtime-wry 2.12.3 にも口がない。確かさ A。
  - 試すために、wry が文脈を作る前に `WebKitWebContext` の class の `set_property` を包み、NULL で来たこの property に meloet の設定を渡す形を `meloet-app` に作った（設定が届いたことは、下の値の差で確かめた）。入れないので消した。
- 上限（memory limit, MB）: `MemoryPressureHandler` の `baseThreshold`。比べる物は `memoryFootprint()` = `/proc/self/statm` の resident、つまり RSS（`wtf/linux/MemoryFootprintLinux.cpp`）。既定は RAM と 3 GB の小さいほう。確かさ A。
- 見る間隔（poll interval）: 既定 30 秒（`s_pollInterval`）。この見はりは、既定のままでもウェブのプロセスで動いている（`WebProcess.cpp` の `setShouldUsePeriodicMemoryMonitor(isFastMallocEnabled() …)`、GTK は `ENABLE_PERIODIC_MEMORY_MONITOR` が ON）。確かさ A。
- conservative（既定 0.33）をこえると、見るたびに `releaseMemory(Critical::No)`: フォント・セレクタ・スタイルなどのキャッシュを捨て、`malloc_trim(0)`。**Linux では GC を呼ばない**（`MemoryRelease.cpp` の `releaseNoncriticalMemory`。GC を呼ぶのは iOS だけ）。確かさ A。
- strict（既定 0.5）をこえると、見るたびに `releaseMemory(Critical::Yes)`: 上に加えて、JIT のコードを全部捨て（`deleteAllCode`）、`garbageCollectSoon()`。これは `Heap::reportAbandonedObjectGraph()` で Full のタイマーを早めるだけで、Full をその場で回さない。前の Full が何も返せなかったとき（a の「13 → 16 MB」）は、タイマーの遅れが無限大のままなので効かない（`FullGCActivityCallback::deathRate` が 0）。確かさ A（ソース）と C（当てはめ）。
- kill（既定 0）: こえると、まず `releaseMemory(Critical::Yes, Synchronous::Yes)`（ここだけが Full の GC をその場で回す）、それでも下がらなければ UI のプロセスがウェブのプロセスを止める（`ProcessTerminationReason::ExceededMemoryLimit`。meloet では `webprocess.rs` が読みなおす）。文書は「0 は止めない」だが、ソースでは 0 のとき `thresholdForMemoryKill()` がプロセスの状態の既定に落ちる。WebKitGTK はプロセスを Active にしない（`setProcessState` を呼ぶ所が glib の側に見つからなかった。確かさ B）ので Inactive の値 = min(3 GB + ページ数 × 1 GB, RAM × 0.9)。**いまの meloet も、RSS が 3 GB をこえると止められて読みなおす**と読める（確かさ C、ためしていない）。本当に止めないには、kill に大きな値（例: 上限 × 1000）を明示する。
- 読んだファイル: `Source/WebKit/UIProcess/API/glib/WebKitMemoryPressureSettings.cpp`、`WebKitWebsiteDataManager.cpp`、`WebKitWebContext.cpp`、`Source/WebKit/UIProcess/glib/WebProcessPoolGLib.cpp`、`Source/WebKit/WebProcess/WebProcess.cpp`、`glib/WebProcessGLib.cpp`、`Source/WTF/wtf/MemoryPressureHandler.{h,cpp}`、`wtf/unix/MemoryPressureHandlerUnix.cpp`、`wtf/linux/MemoryFootprintLinux.cpp`、`Source/WebCore/page/MemoryRelease.cpp`、`bindings/js/GarbageCollectionController.cpp`、`page/OpportunisticTaskScheduler.cpp`、`Source/JavaScriptCore/heap/{Heap,GCActivityCallback,FullGCActivityCallback}.cpp`（いずれも GitHub の `WebKit/WebKit` の `webkitglib/2.52`）。ヘッダは `/usr/include/webkitgtk-4.1/webkit/WebKitMemoryPressureSettings.h`。

### 測った数

- 版: `240f48f` の上の作業ツリー（上のしかけを足した物）。release、窓 1920×1080、バッファ 256、48000 Hz。`JSC_logGC=1`。
- PC: 別の係がほかの作業ツリーで作業中。各回の始めの load average 0.49〜1.03。ヤチヨさんの meloet は動いていない。
- 生の数: `target/r3-mem/m/<回>/`、`target/r3-mem/lat/<回>/`。
- 設定は「上限 MB, conservative, strict, 間隔 秒」。kill は 0。

| 回 | 設定 | 秒 | ピアノロール | Eden / Full | 2 回目の Full（前 → 後 MB） | WebKitWebProcess MB（はじめ → 終わり） |
|---|---|---|---|---|---|---|
| base1 | なし | 120 | なし | 1689 / 1 | — | 337 → 458 |
| exp-never | 3000, 0.33, 0.5, 5（こえない） | 120 | なし | 1598 / 1 | — | 344 → 449 |
| exp1 | 400, 0.8, 0.9, 5 | 120 | なし | 3 / 182 | はじめから | 329 → 358 |
| exp-cons | 1000, 0.33, 0.9, 5 | 120 | なし | 577 / 121 | 20 → 17（すぐ） | 325 → 347 |
| cons2（同じ設定でもう 1 回） | 1000, 0.33, 0.9, 5 | 120 | なし | 457 / 128 | 20 → 17（すぐ） | 326 → 348 |
| exp-cons30 | 1000, 0.33, 0.9, **30** | 120 | なし | 1694 / 1 | — | 337 → 436 |
| exp-late（conservative を 400 MB に） | 1000, 0.4, 0.9, 5 | 180 | なし | 2441 / 1 | — | 340 → 470（90 秒で 400 をこえたが、Full は来ない） |
| exp-strictlate（strict を 380 MB に） | 1000, 0.37, 0.38, 5 | 150 | なし | 1943 / 2 | 136 → 12（終わりぎわ） | 337 → 572（strict に入ってから約 2 MB/秒に速まった） |
| **long-x** | 3000, 0.0667, 0.5, 5 | 600 | なし | 546 / 763 | 20 → 17（すぐ） | **325 → 357（いちばん大きい 362）** |
| **long-x-pr** | 3000, 0.0667, 0.5, 5 | 600 | ひらいたまま | 7244 / 13 | 362 → 21（終わりぎわ） | **334 → 798（いちばん大きい 815）** |
| exp-p1-pr | 3000, 0.0667, 0.5, **1** | 150 | ひらいたまま | 1986 / 1 | — | 333 → 485 |

long-x と long-x-pr の 60 秒ごと（MB）:

| 秒 | 60 | 120 | 180 | 240 | 300 | 360 | 420 | 480 | 540 | 600 |
|---|---|---|---|---|---|---|---|---|---|---|
| long-x | 345 | 348 | 354 | 357 | 353 | 355 | 355 | 355 | 355 | 355 |
| long-x-pr | 391 | 435 | 489 | 535 | 578 | 619 | 665 | 706 | 751 | 798 |

- どの回も、はじめの Full は「13 → 16 MB」（何も返せない）。効いた回は、そのすぐあとに 2 回目の Full（20 → 17 MB）が来て、そこから Full が 1 秒に約 1 回つづき、Eden はほとんど回らない。効かなかった回は 2 回目が来ない（a と同じ形）。
- 効いたのは、conservative をはじめから（RSS がいつも上に）こえていて、間隔が 5 秒の回だけ。30 秒の間隔、あとからこえる conservative、あとからこえる strict は、どれも Full をよびもどさなかった。ピアノロールをひらいたままでは、5 秒でも 1 秒でも効かなかった。
- 見立て（確かさ C）: Full がまわりつづけるかどうかは、起動の直後の数秒に決まる。conservative の片づけ（キャッシュを捨てる、`malloc_trim`）が、なぜ 2 回目の Full を起こすのかは、ソースからは説明できなかった（Linux の conservative は GC を呼ばない）。ピアノロールをひらくと、その数秒の割り当てや GC の順が変わると読む。
- strict は害がある: 見るたびに JIT のコードを全部捨てるので、作りなおしで RSS が速くふえた（exp-strictlate）。

### 主スレッドの止まりと鍵 → 画面（録音中、mixed、ピアノロール、600 鍵）

`tools/measure/latency.sh --no-build --state recording --pattern mixed --piano-roll --count 600`。設定ありの回は `JSC_logGC=1` もつけた（なしの回はつけていない）。設定ありの回は、ピアノロールありなのに Full が回った（Full 233 / Eden 31）。

| | 間をあけた鍵 n | Rust → 描いた p50 / p95 / 最大 | 2 コマの線 | すぐあとの鍵 n | p50 / p95 / 最大 | 3 コマの線 | 25 ms をこえたコマの間（はじめの 4000 コマ、約 55 秒） |
|---|---|---|---|---|---|---|---|
| 設定なし（base-rec） | 427 | 0.91 / 11.79 / 15.82 | 100 % | 173 | 5.44 / 11.57 / 14.20 | 100 % | 4（起動とピアノロールをひらく所だけ） |
| 設定あり（x-rec） | 434 | 0.69 / 10.92 / **34.00** | **1 鍵こえた** | 166 | 6.38 / 13.40 / 17.66 | 100 % | 8（上の 4 と、4.9・8.2・27.8・40.6 秒に 38〜42 ms） |

- Full の GC 1 回の止まり（logGC の `p=`）は、ふだん 1〜3 ms（long-x で 762 回の p50 1.2、p95 2.3）。ただし 30 ms をこえる回がときどきある（long-x で 16.7 ms 以上が 2 回、最大 32.8 ms。x-rec では 5 回）。38〜42 ms のコマの間と 34 ms の鍵は、これと重なると読む（確かさ C、時刻は合わせていない）。
- Eden にも 30 ms ほどの止まりがときどきある（設定なしの 120 秒で 16.7 ms 以上が 3〜13 回、最大 35〜38 ms）ので、長い止まりは Full だけの物ではない。設定なしの latency の回は、たまたま最初の 55 秒に当たらなかった。2 コマの線をこえるかどうかは、1 回ずつでは決められない。
- 録音中に起きにくくする手（しきい値・間隔）は、上のとおり「はじめに conservative をこえている」ことがいるので、録音中だけ止める形が作れない。

### 棟梁に（次の手の案）

1. このまま入れない（起動の運しだいで、ピアノロールをひらいていると効かない）。
2. どの回でも、Full が 1 秒に 1 回まわりはじめると RSS は平らで、止まりもふだん 1〜3 ms だった。Full を確かに起こす別の口（e の 2 の iframe、または 3 の web extension）を試すねうちはある。
3. 3 GB で WebKit がウェブのプロセスを止める（kill の既定）と読めるので、どの手をとるにしても、kill を明示するかどうかは決めたほうがよい。

## g. JavaScriptCore の Options（環境変数 `JSC_<名前>`、2026-11-15 夜）

**入れた: `JSC_useGenerationalGC=0`。** ピアノロールをひらいたままの 10 分の録音で、WebKitWebProcess が 347 → 410 MB（いちばん大きい 414）。鍵 → 画面の 2 コマの線は、ログなしの 2 回とも守った。

### ソースで読んだ事（`WebKit/WebKit` の `webkitglib/2.52`。入れた版は 2.52.6。確かさ A）

- 環境変数: JSC は初めの一度、`environ` の `JSC_` で始まる物を全部 `Options::setOption` にかける（`runtime/Options.cpp` の `Options::initialize`）。Normal の物は release でも効く。Restricted は `restrictedOptionsEnabled` のときだけ。下の物はどれも Normal。名前のまちがいは `ERROR: invalid option` と書くだけ（`validateOptions` が立っていなければ落ちない）。ウェブのプロセスは meloet の環境を受けつぐ（前の係の `JSC_logGC=1` で確かめ済み）。
- `runtime/OptionsList.h`:

  | 名前 | 型 | 既定 | 種類 | 使う所と読み |
  |---|---|---|---|---|
  | `useGenerationalGC` | Bool | true | Normal | `Heap::shouldDoFullCollection` が最初に見る。false なら、どの GC も、Eden を頼まれても Full |
  | `gcMaxHeapSize` | Unsigned（バイト） | 0 | Normal | 0 でないと、割り当てで GC を頼むしきいが「この周の割り当て > これ」だけになる（`Heap::collectIfNecessaryOrDefer`）。ヒープの上限ではない |
  | `percentCPUPerMBForFullTimer` | Double | 0.0003125 | Normal | Full のタイマーの遅れ = 前の Full の長さ ÷ min(返せる見こみ MB × これ, `collectionTimerMaxPercentCPU` 0.05)。返せる見こみ = ふえた量 × 前の Full で返せた割合（`GCActivityCallback::didAllocate`）。割合が 0 だと遅れは無限大のままで、この値では直らない |
  | `minHeapUtilization` | Double | 0.8 | Normal | `heap/` の Heap.cpp・GCActivityCallback には出てこない（Full を決める所ではない） |
  | `largeHeapGrowthFactor` ほか、`heapGrowthMaxIncrease` 3.0、`heapGrowthSteepnessFactor` 2.0 | Double | — | Normal | Full のあとの上限（`proportionalHeapSize`）。RAM 16 GB 以上は Aggressive で `heapGrowthMaxIncrease` 側 |
  | `criticalGCMemoryThreshold` | Double | 0.80 | Normal | PC のメモリの使用率がこえると、頼み先の決まっていない GC が Full になる |
  | `collectContinuously` | Bool | false | Normal | 試していない（試験用で、止まりが増える） |

- なぜ Full が来ないか（`runtime/VM.cpp` の `performOpportunisticallyScheduledTasks`、`heap/Heap.cpp` の `updateAllocationLimits`）: ひまな時の GC は、`m_shouldDoOpportunisticFullCollection` が立っていなければ `collectSync(CollectionScope::Eden)` と**種類を決めて**頼む。Eden のあと「残りが 1/3 より少ない」と `m_shouldDoFullCollection` が立つが、これを見るのは種類を決めていない頼み（割り当てがしきいをこえたとき）だけ。ひまな時の Eden が 1 秒に十数回まわると、割り当ては周ごとに小さく、しきい（Full のあとのヒープ 16 MB からの見こみで約 48 MB。計算: 16 × (3 × e^(−2 × 16 MB ÷ RAM) + 1) − 16。確かさ C）にとどかないので、その旗は使われない。`useGenerationalGC=0` は `shouldDoFullCollection` の最初で true を返すので、種類を決めた Eden の頼みも Full になる。確かさ A（ソース）と C（meloet での当てはめ）。

### 測った数（150 秒の録音、ピアノロールをひらいたまま、`JSC_logGC=1`）

- 版: `f72d8e1` の上の作業ツリー。release、窓 1920×1080、バッファ 256、48000 Hz。`tools/measure/long-record.sh --no-build --seconds 150 --piano-roll`。
- PC: ほかの係なし。始めの load average 0.69〜1.18。ヤチヨさんの meloet は動いていない（vite だけ）。
- 生の数: `target/r3-mem/m3/<回>/`。止まりは logGC の `p=`（その GC で主スレッドが止まった合計）。

| 回 | 設定 | Eden / Full | 止まり p50 / p95 / 最大 ms（Full） | 16.7 ms をこえた GC | ヒープ MB（終わり） | WebKitWebProcess MB（10 秒 → 180 秒） |
|---|---|---|---|---|---|---|
| base | なし | 287 / 269 | 1.27 / 2.38 / 5.53 | 1 | 20 | 350 → 393（Full が回りはじめた運のよい回） |
| base2 | なし | 262 / 262 | 1.29 / 2.40 / 3.27 | 3 | 20 | 348 → 394（同） |
| base3 | なし | 1979 / 1 | Eden 0.80 / 1.11 / 35.15 | 13 | 96 | 350 → **506** |
| nogen | `useGenerationalGC=0` | 0 / 538 | 1.26 / 2.33 / 32.84 | 2 | 20 | 347 → 390 |
| nogen2 | 同 | 0 / 545 | 1.27 / 2.30 / 32.80 | 2 | 20 | 347 → 389 |
| nogen3 | 同 | 0 / 538 | 1.25 / 2.23 / 32.99 | 1 | 20 | 348 → 390 |
| max4m | `gcMaxHeapSize=4194304` | 3715 / 1 | Eden 0.71 / 0.96 / 33.05 | 12 | 93 | 349 → **502** |
| max16m | `gcMaxHeapSize=16777216` | 2098 / 1 | Eden 0.81 / 1.10 / 31.62 | 13 | 97 | 356 → **500** |

- 設定なしは、3 回のうち 2 回が「Full が回りはじめる」形（f と同じく運しだい）。`useGenerationalGC=0` は 3 回とも同じ形で、Full が 1 秒に約 3 回、1 回の止まりはふだん 1〜3 ms。
- `gcMaxHeapSize` は悪くなった: 割り当てで頼む GC がふえても Eden のまま（Full の旗が立つ前に周がおわる）で、Full は来ない。
- xrun: どの回も録音の前（1〜3 秒目）の 1 回だけ。

### 10 分（録音、ピアノロールをひらいたまま、`useGenerationalGC=0`、`JSC_logGC=1`）

- `target/r3-mem/m3/long-nogen-pr/`。始めの load average 0.73。テイクの鍵 2106（ノートの一覧 2106 を絵で確かめた）。xrun は 3 秒目に 1 回だけ（録音の前）。
- Eden 0 / Full 1626、止まり p50 1.30 / p95 2.38 / 最大 33.01 ms（16.7 ms をこえたのは 1 回）。ヒープは 20 MB 前後のまま（いちばん大きい 23）。

| 秒 | 10 | 60 | 120 | 180 | 240 | 300 | 360 | 420 | 480 | 540 | 600 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| WebKitWebProcess MB | 347 | 376 | 384 | 393 | 401 | 402 | 404 | 406 | 404 | 410 | 410 |
| （くらべ）c のひらいたまま | — | 406 | 451 | 507 | 556 | 597 | 641 | 682 | 728 | 781 | 826 |

- 240 秒からあとは 6 分で 9 MB（約 0.025 MB/秒。計算: (410 − 401) ÷ 360）。はじめの 4 分の 54 MB は、ピアノロールと録っているテイクがのびる分と読む（確かさ C）。

### 鍵 → 画面（録音中、mixed、ピアノロール、600 鍵）

`tools/measure/latency.sh --no-build --state recording --pattern mixed --piano-roll --count 600`。設定は、入れたあとの meloet（既定で `useGenerationalGC=0`）と、外から `JSC_useGenerationalGC=1` を決めた回（外の値が勝つことの確かめも兼ねる）。生の数: `target/r3-mem/lat3/<回>/`。始めの load average 0.55〜1.11。

| 回 | GC | logGC | 間をあけた鍵 p50 / p95 / 最大 | 2 コマの線 | すぐあとの鍵 p50 / p95 / 最大 | 3 コマの線 | 25 ms をこえたコマの間（3 秒より後、はじめの 4000 コマ） |
|---|---|---|---|---|---|---|---|
| q-nogen1 | Full だけ | なし | 1.36 / 12.24 / 16.49 | 守った | 6.78 / 13.82 / 17.17 | 守った | 0 |
| q-nogen2 | Full だけ | なし | 0.72 / 10.09 / 15.75 | 守った | 6.07 / 12.87 / 19.18 | 守った | 0 |
| q-gen1 | 世代あり | なし | 0.84 / 11.20 / 14.36 | 守った | 5.40 / 13.21 / 17.19 | 守った | 0 |
| q-gen2 | 世代あり | なし | 0.70 / 12.07 / 15.10 | 守った | 6.12 / 13.67 / 16.22 | 守った | 0 |
| nogen1 | Full だけ（0 / 451） | あり | 0.85 / 11.46 / 29.52 | 1 鍵こえた | 5.84 / 12.64 / 16.81 | 守った | 3（39〜46 ms） |
| nogen2 | Full だけ（0 / 446） | あり | 0.86 / 10.44 / 16.08 | 守った | 6.22 / 13.05 / 18.06 | 守った | 1（43 ms） |
| gen1 | 世代あり（204 / 213） | あり | 0.83 / 10.51 / 22.06 | 1 鍵こえた | 6.30 / 13.33 / 15.85 | 守った | 5（38〜47 ms） |
| gen2 | 世代あり（84 / 226） | あり | 1.14 / 11.61 / 29.49 | 1 鍵こえた | 6.47 / 12.79 / 18.21 | 守った | 3（42〜52 ms） |

- `JSC_logGC=1` をつけた回だけ、40 ms ほどのコマの間と 2 コマをこえる鍵が出た（設定によらない）。logGC の約 33 ms の止まり（どの形でも 150 秒に 1〜13 回）は、ログを書くことそのものの重さをふくむと読む（確かさ C）。ログなしの 4 回は、どちらの設定でも 3 秒より後に 25 ms をこえたコマの間がなかった。
- 鍵 → 音は 4 回とも同じ（Rust → コールバック p95 5.0〜5.1 ms）。
- 世代ありの latency の回は、55 秒の間に Full が回りはじめた（運のよい形）ので、「世代ありでふえつづける形」とのくらべにはなっていない。ログなしの回は、どちらの形だったかわからない。

### 入れた形

- `crates/meloet-app/src/main.rs` の `JSC_OPTIONS`（1 か所の定数）。`main` のいちばん初め（スレッドができる前）に、環境にまだないときだけ決める。**外から同じ変数が決められていたら、そちらを使う**（くらべるため。上の gen の回で、外の `=1` が効いて Eden が回ったことを確かめた）。
- 止める道: `JSC_useGenerationalGC=1` をつけて起動する。

### 確かめていない事

- 1 時間の録音。ログなしの長い録音の RSS（10 分は logGC つき）。
- Full だけの GC が、曲が大きくなったとき（ノートが数万）に 1 回何 ms になるか。止まりは生きている物の数に比例するので、曲が大きいと重くなる（確かさ C）。いまのヒープは 20 MB 前後。
- e の 2（iframe）と 3（web extension）は作らなかった。3 は、公開の `JSGarbageCollect` が `reportAbandonedObjectGraph()` だけ（Full のタイマーを早めるだけで、f の strict と同じく効かない見こみ。`API/JSBase.cpp`）。その場で Full を回すのは `JSSynchronousGarbageCollectForDebugging`（入れた libjavascriptcoregtk-4.1 2.52.6 の `nm -D` で外に出ている。ヘッダにはない、試験用の名前）。web extension（別の `.so`、wry に口がなく `webkit_web_context_set_web_extensions_directory` をウェブのプロセスより先に呼ぶしかけがいる）と合わせて、手間は大きい。
